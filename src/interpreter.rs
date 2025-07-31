use std::{
    collections::{hash_map::Entry, HashMap, VecDeque},
    convert::identity,
    rc::Rc, sync::Arc,
};

use itertools::Itertools;
use num_enum::TryFromPrimitive;
use owo_colors::OwoColorize;
use unicode_segmentation::UnicodeSegmentation;

use crate::{codegen::{BlockLibrary, BlockRuntimeLogic}, interpreter::{
    id::Id,
    opcode::{BuiltinProcedure, Opcode, Trigger},
    value::{EventValue, ProcedureValue, Value, VarState},
}};

pub mod id;
pub mod opcode;
pub mod value;

pub struct Program {
    constants: Box<[Value]>,
    vars: Vec<VarState>,
    procedures: Vec<Rc<ProcedureValue>>,
    builtins: Vec<Option<BlockRuntimeLogic>>,
    events: Vec<EventValue>,
    task_queue: VecDeque<Task>,
    triggers: HashMap<Trigger, Vec<Rc<ProcedureValue>>>,
}

impl Program {
    pub fn new(constants: Box<[Value]>, vars: Vec<VarState>, builtins: Vec<Option<BlockRuntimeLogic>>) -> Self {
        Self {
            constants,
            vars,
            procedures: Vec::new(),
            builtins,
            events: Vec::new(),
            task_queue: VecDeque::new(),
            triggers: HashMap::new(),
        }
    }

    pub fn register(&mut self, procedure: impl Into<Rc<ProcedureValue>>) -> Rc<ProcedureValue> {
        let proc: Rc<ProcedureValue> = procedure.into();
        proc.ident
            .set(self.procedures.len().into())
            .expect("procedure id settable");
        self.procedures.push(proc.clone());
        proc
    }

    pub fn register_event(&mut self, name: impl Into<Arc<str>>) -> Id<EventValue> {
        let idx = self.events.len();
        self.events.push(EventValue::new(name));
        idx.into()
    }

    pub fn add_trigger(&mut self, proc: Rc<ProcedureValue>, trigger: Trigger) {
        match self.triggers.entry(trigger) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().push(proc);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![proc]);
            }
        }
    }

    pub fn dispatch(&mut self, trigger: Trigger) {
        let handler_procedures = self
            .triggers
            .get(&trigger)
            .map_or([].as_slice(), Vec::as_slice);

        let tasks = handler_procedures.iter().cloned().map(Task::new);
        self.task_queue.extend(tasks);
    }

    pub fn enqueue(&mut self, task: Task) {
        self.task_queue.push_back(task);
    }

    pub fn run(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            task.run_until_yield(self);
            if !task.is_complete() {
                self.task_queue.push_back(task);
            }
        }
    }

    pub fn dbg_string(&self, value: &Value) -> Arc<str> {
        match value {
            &Value::Procedure(id) => {
                let proc_name = self
                    .procedures
                    .get(id.get())
                    .map_or("{unknown}", |proc| proc.name());

                format!("procedure {id:?} {proc_name:?}").into()
            }
            &Value::ReturnLocation(location) => format!("loc 0x{location:X?}").into(),
            &Value::Event(id) => {
                let event = self.events.get(id.get());
                format!("event {id:?} {:?}", event.map_or("{unknown}", |e| e.name())).into()
            }
            other => other.cast_string(),
        }
    }

    pub fn read_var(&mut self, id: Id<VarState>) -> Value {
        self.vars[id.get()].as_ref().borrow().clone()
    }

    pub fn set_var(&mut self, id: Id<VarState>, value: Value) {
        *self.vars[id.get()].as_ref().borrow_mut() = value;
    }
}

pub struct Task {
    procedure: Rc<ProcedureValue>,
    location: usize,
    scopes: Vec<Box<[Value]>>,
    stack: Vec<Value>,
    complete: bool,
}

impl Task {
    pub fn new(procedure: Rc<ProcedureValue>) -> Self {
        assert_eq!(procedure.param_count, 0);
        let scope = vec![Value::default(); procedure.locals.len()];

        Self {
            procedure,
            location: 0,
            scopes: vec![scope.into_boxed_slice()],
            stack: Vec::with_capacity(10),
            complete: false,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn enter_scope(&mut self, scope: Box<[Value]>) {
        self.scopes.push(scope);
    }

    pub fn leave_scope(&mut self) -> Option<Box<[Value]>> {
        self.scopes.pop()
    }

    fn read_local(&self, idx: u32) -> Value {
        let scope = self.scopes.last().unwrap();
        scope[idx as usize].clone()
    }

    fn set_local(&mut self, idx: u32, value: Value) {
        let scope = self.scopes.last_mut().unwrap();
        scope[idx as usize] = value;
    }

    fn read_immediate(&mut self) -> u32 {
        let imm = self.procedure.bytecode()[self.location];
        self.location += 1;
        imm
    }

    fn read_opcode(&mut self) -> Opcode {
        Opcode::try_from_primitive(self.read_immediate()).unwrap()
    }

    fn read_id<T>(&mut self) -> Id<T> {
        Id::from(self.read_immediate() as usize)
    }

    fn pop_n_and_map<const N: usize, T>(&mut self, map: impl FnMut(Value) -> T) -> [T; N] {
        let first_idx = self.stack.len() - N;
        self.stack
            .drain(first_idx..self.stack.len())
            .map(map)
            .collect_array::<N>()
            .unwrap()
    }

    fn pop_values<const N: usize>(&mut self) -> [Value; N] {
        self.pop_n_and_map(identity)
    }

    fn pop_numbers<const N: usize>(&mut self) -> [f64; N] {
        self.pop_n_and_map(|v| v.cast_number())
    }

    fn pop_strings<const N: usize>(&mut self) -> [Arc<str>; N] {
        self.pop_n_and_map(|v| v.cast_string())
    }

    fn run_until_yield(&mut self, program: &mut Program) {
        loop {
            if self.location >= self.procedure.bytecode().len() {
                panic!("Reached end of procedure bytecode without returning");
            }

            let should_yield = self.run_opcode(program);
            if should_yield {
                break;
            }
        }
    }

    fn run_opcode(&mut self, program: &mut Program) -> bool {
        let opcode = self.read_opcode();

        let debug_message = format!(
            "$ {opcode:?} proc={:?} stack={:?}",
            self.procedure.name(),
            self.stack,
        );
        println!("{}", debug_message.bright_black());

        match opcode {
            Opcode::PushConstant => {
                let imm = self.read_immediate() as usize;
                let constant = program.constants[imm].clone();
                self.stack.push(constant);
            }
            Opcode::PushZero => {
                self.stack.push(Value::Number(0.0));
            }
            Opcode::PushUInt32 => {
                let uint = self.read_immediate();
                self.stack.push(Value::Number(uint as f64));
            }

            Opcode::DispatchEvent => {
                let id = Id::<EventValue>::from(self.read_immediate() as usize);
                let dbg_msg = format!("> {}", program.dbg_string(&id.into()));
                println!("  {}", dbg_msg.bright_black());
                program.dispatch(Trigger::Event(id));
                return true;
            }
            Opcode::CallBuiltin => {
                let imm = self.read_immediate();

                let mut builtin = program.builtins.remove(imm as usize);
                if let Some(builtin) = builtin.as_deref_mut() {
                    builtin(RuntimeContext { task: self, program });
                } else {
                    unimplemented!("runtime logic for builtin {imm}");
                }

                return true;
            }
            Opcode::CallProcedure => {
                let proc_id = self.read_immediate() as usize;
                let procedure = program.procedures[proc_id].clone();

                let mut scope = Vec::with_capacity(procedure.locals.len());
                // Add locals initialized from parameters in the stack
                for _ in 0..procedure.param_count {
                    scope.push(self.stack.pop().unwrap());
                }
                // Add uninitialized locals
                while scope.len() < procedure.locals.len() {
                    scope.push(Value::default());
                }

                // Save return location
                self.stack.extend([
                    Value::ReturnLocation(self.location),
                    self.procedure.as_value(),
                ]);

                // Switch contexts
                self.location = 0;
                self.procedure = procedure;
                self.enter_scope(scope.into_boxed_slice());
            }

            Opcode::Jump => {
                self.location = self.read_immediate() as usize;
            }
            Opcode::JumpIfTrue => {
                let location = self.read_immediate() as usize;
                let condition = self.stack.pop().unwrap();
                if condition.cast_boolean() {
                    self.location = location;
                }
            }
            Opcode::JumpIfFalse => {
                let location = self.read_immediate() as usize;
                let condition = self.stack.pop().unwrap();
                if !condition.cast_boolean() {
                    self.location = location;
                }
            }
            Opcode::Return => {
                let Some(procedure_id) = self.stack.pop() else {
                    // Returning from the root procedure
                    self.complete = true;
                    return true;
                };

                // Restore context from stack
                let procedure_id = procedure_id.unwrap_procedure();

                self.leave_scope();
                self.procedure = program.procedures[procedure_id.get()].clone();
                self.location = self.stack.pop().unwrap().unwrap_return_location();
            }
            Opcode::Yield => {
                return true;
            }

            Opcode::SetVar => {
                let new_value = self.stack.pop().unwrap();
                program.set_var(self.read_id::<VarState>(), new_value);
            }
            Opcode::ClearVar => {
                program.set_var(self.read_id::<VarState>(), Value::default());
            }
            Opcode::ZeroVar => {
                program.set_var(self.read_id::<VarState>(), Value::Number(0.0));
            }
            Opcode::PushVar => {
                let id = self.read_id::<VarState>();
                self.stack.push(program.read_var(id));
            }

            Opcode::SetLocal => {
                let idx = self.read_immediate();
                let value = self.stack.pop().unwrap();
                self.set_local(idx, value);
            }
            Opcode::PushLocal => {
                let idx = self.read_immediate();
                self.stack.push(self.read_local(idx));
            }
            Opcode::DecLocal => {
                let idx = self.read_immediate();
                let old = self.read_local(idx).cast_number();
                self.set_local(idx, Value::Number(old - 1.0));
            }

            Opcode::Add => {
                let [left, right] = self.pop_numbers::<2>();
                let result = left + right;
                self.stack.push(result.into());
            }

            Opcode::GreaterThan => {
                let [left, right] = self.pop_numbers::<2>();
                self.stack.push(Value::Boolean(left > right));
            }

            other => {
                todo!("{other:?}")
            }
        }

        false
    }

    fn run_builtin(&mut self, procedure: BuiltinProcedure, program: &mut Program) {
        match procedure {
            BuiltinProcedure::Say => {

            }
            BuiltinProcedure::LengthOf => {
                let param = self.stack.pop().unwrap();
                let length = param.cast_string().len();
                self.stack.push(Value::Number(length as f64));
            }
            BuiltinProcedure::LetterOf => {
                let [index, string] = self.pop_values();
                let string = string.cast_string();
                let letter = string
                    .graphemes(true)
                    .nth(index.cast_number() as usize - 1)
                    .unwrap_or("");
                self.stack.push(Value::String(letter.into()));
            }
            BuiltinProcedure::Join => {
                let [left, right] = self.pop_strings();
                self.stack.push(format!("{left}{right}").into());
            }
            _ => todo!("{procedure:?}"),
        }
    }
}

pub struct RuntimeContext<'a> {
    task: &'a mut Task,
    program: &'a mut Program,
}

impl RuntimeContext<'_> {
    pub fn stack(&self) -> &Vec<Value> {
        &self.task.stack
    }

    pub fn stack_mut(&mut self) -> &mut Vec<Value> {
        &mut self.task.stack
    }

    pub fn program(&self) -> &Program {
        self.program
    }

    pub fn program_mut(&mut self) -> &mut Program {
        self.program
    }
}
