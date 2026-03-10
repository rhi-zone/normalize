// Typed code — no `any` annotations

function parse(input: string): unknown {
  return JSON.parse(input);
}

function identity<T>(x: T): T {
  return x;
}

interface Options {
  callback: (event: MouseEvent) => void;
  data: Record<string, string>;
}

let value: number = 42;
let items: string[] = ["a", "b"];

function processData(data: unknown[]): void {
  data.forEach((item) => {
    console.log(item);
  });
}
