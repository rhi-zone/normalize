// Explicit `any` in type annotations — disables type checking

function parse(input: any): any {
  return JSON.parse(input);
}

function processData(data: any[]): void {
  data.forEach((item: any) => {
    console.log(item);
  });
}

let value: any = 42;

interface Options {
  callback: (event: any) => void;
  data: any;
}
