use std::{
    cmp::Reverse, collections::{hash_map::Entry, BinaryHeap, HashMap, VecDeque}, convert::identity, rc::Rc, sync::Arc, thread::sleep, time::{Duration, Instant}
};

use itertools::Itertools;
use num_enum::TryFromPrimitive;
use owo_colors::OwoColorize;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    ast::Target,
    blocks::{BlockRuntimeLibrary, BlockRuntimeLogic},
    interpreter::{
        id::Id,
        opcode::{BuiltinProcedure, Opcode, Trigger},
        value::{EventValue, ProcedureValue, Value, VarState},
    },
};

pub mod id;
pub mod opcode;
pub mod value;

#[derive(Debug)]
pub struct Program {
    constants: Box<[Value]>,
    global_vars: Vec<VarState>,
    procedures: Vec<Rc<ProcedureValue>>,
    builtins: Option<BlockRuntimeLibrary>,
    events: Vec<EventValue>,
    triggers: HashMap<Trigger, Vec<Rc<ProcedureValue>>>,
    targets: Vec<TargetScope>,

    /// A queue of tasks that must be scheduled before this frame is over.
    task_queue: VecDeque<Task>,
    /// A list of tasks that are inactive or waiting for the next frame.
    sleepers: BinaryHeap<Reverse<Sleeper>>,
}

impl Program {
    pub fn new(
        builtins: BlockRuntimeLibrary,
        constants: Box<[Value]>,
        events: Vec<EventValue>,
        global_vars: Vec<VarState>,
        targets: Vec<TargetScope>,
    ) -> Self {
        Self {
            constants,
            global_vars,
            procedures: Vec::new(),
            builtins: Some(builtins),
            events,
            triggers: HashMap::new(),
            targets,
            task_queue: VecDeque::new(),
            sleepers: BinaryHeap::new(),
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

    pub fn has_incomplete_tasks(&self) -> bool {
        !self.sleepers.is_empty() || !self.task_queue.is_empty()
    }

    pub fn next_wake(&self) -> Instant {
        self.sleepers
            .peek()
            .map_or_else(Instant::now, |s| s.0.0.wake_time)
    }

    fn wake_sleepers(&mut self, now: Instant) {
        while let Some(Reverse(Sleeper(sleeper))) = self.sleepers.peek()
            && sleeper.wake_time <= now
        {
            let Reverse(Sleeper(task)) = self.sleepers.pop().unwrap();
            self.enqueue(task);
        }
    }

    /// Enqueues tasks that are done sleeping, then runs the interpreter
    /// until all tasks are sleeping again. Tasks are sent to sleep whenever
    /// they yield or wait for a duration of time.
    pub fn run_frame(&mut self) {
        let frame_start = Instant::now();

        let wake_time = self.next_wake();
        if let Some(delay) = wake_time.checked_duration_since(frame_start) {
            sleep(delay);
        }

        self.wake_sleepers(wake_time);

        let mut next_priority = frame_start;

        while let Some(mut task) = self.task_queue.pop_front() {
            // Wake Time doubles as task priority because it's used to order
            // the tasks in the sleepers queue. Here we reset the priority
            // to ensure all tasks maintain a predictable execution order.
            task.wake_time = next_priority;
            next_priority += Duration::from_nanos(1);

            task.run_until_yield(self);

            if !task.is_complete() {
                self.sleepers.push(Reverse(Sleeper(task)));
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

    pub fn read_var(&mut self, target_id: usize, id: Id<VarState>) -> Value {
        let target = &self.targets[target_id];
        let idx = id.get();

        if let Some(idx) = idx.checked_sub(self.global_vars.len()) {
            target.vars[idx].as_ref().borrow().clone()
        } else {
            self.global_vars[idx].as_ref().borrow().clone()
        }
    }

    pub fn set_var(&mut self, target_id: usize, id: Id<VarState>, value: Value) {
        self.with_var(target_id, id, |var| *var = value);
    }

    pub fn with_var(&mut self, target_id: usize, id: Id<VarState>, cb: impl FnOnce(&mut Value)) {
        let target = &mut self.targets[target_id];
        let idx = id.get();

        if let Some(idx) = idx.checked_sub(self.global_vars.len()) {
            cb(&mut *target.vars[idx].as_ref().borrow_mut());
        } else {
            cb(&mut *self.global_vars[idx].as_ref().borrow_mut());
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Task {
    procedure: Rc<ProcedureValue>,
    location: usize,
    scopes: Vec<Box<[Value]>>,
    stack: Vec<Value>,
    complete: bool,
    wake_time: Instant,
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
            wake_time: Instant::now(),
        }
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn sleep_until(&mut self, wake_time: Instant) {
        self.wake_time = wake_time;
    }

    pub fn is_done_sleeping(&self) -> bool {
        Instant::now() >= self.wake_time
    }

    pub fn stack(&self) -> &Vec<Value> {
        &self.stack
    }

    pub fn stack_mut(&mut self) -> &mut Vec<Value> {
        &mut self.stack
    }

    fn pop_n_and_map<const N: usize, T>(&mut self, map: impl FnMut(Value) -> T) -> [T; N] {
        let first_idx = self.stack.len() - N;
        self.stack
            .drain(first_idx..self.stack.len())
            .map(map)
            .collect_array::<N>()
            .unwrap()
    }

    pub fn pop_values<const N: usize>(&mut self) -> [Value; N] {
        self.pop_n_and_map(identity)
    }

    pub fn pop_numbers<const N: usize>(&mut self) -> [f64; N] {
        self.pop_n_and_map(|v| v.cast_number())
    }

    pub fn pop_strings<const N: usize>(&mut self) -> [Arc<str>; N] {
        self.pop_n_and_map(|v| v.cast_string())
    }

    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
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

    fn run_until_yield(&mut self, program: &mut Program) {
        // Wake time is used as priority, so reset this task's priority to
        // send it to the back of the queue because we are running it.
        self.wake_time = Instant::now();

        loop {
            if self.location >= self.procedure.bytecode().len() {
                panic!("Reached end of procedure bytecode without returning");
            }

            let did_yield = self.run_opcode(program);
            if did_yield {
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
            Opcode::PushNumber => {
                let bytes_0 = self.read_immediate();
                let bytes_1 = self.read_immediate();

                let bytes = bytemuck::cast([bytes_0, bytes_1]);
                let num = f64::from_le_bytes(bytes);

                self.stack.push(Value::Number(num));
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

                let mut library = program
                    .builtins
                    .take()
                    .expect("builtins library should be available");

                if let Some(builtin) = library.get(imm as usize) {
                    builtin(RuntimeContext {
                        task: self,
                        program,
                    });
                } else {
                    unimplemented!("runtime logic for builtin {imm}");
                }

                program.builtins = Some(library);

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
            Opcode::Sleep => {
                let [duration_secs] = self.pop_numbers();
                self.wake_time = Instant::now() + Duration::from_secs_f64(duration_secs);
                return true;
            }

            Opcode::SetVar => {
                let new_value = self.stack.pop().unwrap();
                program.set_var(
                    self.procedure.target_id,
                    self.read_id::<VarState>(),
                    new_value,
                );
            }
            Opcode::ChangeVar => {
                let offset = self.stack.pop().unwrap();
                program.with_var(
                    self.procedure.target_id,
                    self.read_id::<VarState>(),
                    |var| {
                        *var = Value::Number(var.cast_number() + offset.cast_number());
                    },
                );
            }
            Opcode::ClearVar => {
                program.set_var(
                    self.procedure.target_id,
                    self.read_id::<VarState>(),
                    Value::default(),
                );
            }
            Opcode::ZeroVar => {
                program.set_var(
                    self.procedure.target_id,
                    self.read_id::<VarState>(),
                    Value::Number(0.0),
                );
            }
            Opcode::PushVar => {
                let id = self.read_id::<VarState>();
                self.stack
                    .push(program.read_var(self.procedure.target_id, id));
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
            BuiltinProcedure::Say => {}
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

#[derive(Debug)]
struct Sleeper(Task);

impl Eq for Sleeper {}

impl PartialEq for Sleeper {
    fn eq(&self, other: &Self) -> bool {
        self.0.wake_time == other.0.wake_time
    }
}

impl Ord for Sleeper {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.wake_time.cmp(&other.0.wake_time)
    }
}

impl PartialOrd for Sleeper {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub struct TargetScope {
    vars: Vec<VarState>,
}

impl TargetScope {
    pub const fn new(vars: Vec<VarState>) -> Self {
        Self { vars }
    }
}

impl From<&Target> for TargetScope {
    fn from(value: &Target) -> Self {
        Self::new(value.variables.values().map(|v| v.initialize()).collect())
    }
}

pub struct RuntimeContext<'a> {
    task: &'a mut Task,
    program: &'a mut Program,
}

impl RuntimeContext<'_> {
    pub const fn task(&self) -> &Task {
        self.task
    }

    pub const fn task_mut(&mut self) -> &mut Task {
        self.task
    }

    pub const fn program(&self) -> &Program {
        self.program
    }

    pub const fn program_mut(&mut self) -> &mut Program {
        self.program
    }
}
