// function type_percent_s(text) {
//     globalThis.textsofar = "";
//     globalThis.c = 0;

//     const length = text.length;
//     for (var i = 0; i < length; i++) {
//         globalThis.c += 1;
//         console.log(globalThis.textsofar + text[globalThis.c]);
//         globalThis.textsofar = globalThis.textsofar + text[globalThis.c];
//     }
// }

// type_percent_s("hello world")

var textsofar, c;

const text = "helloWorld";

textsofar = "";
c = 0;

const length = text.length;
for (var i = 0; i < length; i++) {
    c += 1;
    console.log(textsofar + text[c]);
    textsofar = textsofar + text[c];
}
