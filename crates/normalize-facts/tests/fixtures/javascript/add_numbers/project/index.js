const { Calculator } = require("./math");

const calc = new Calculator();
console.log(calc.compute("add", 2, 3));
console.log(calc.compute("mul", 4, 5));
