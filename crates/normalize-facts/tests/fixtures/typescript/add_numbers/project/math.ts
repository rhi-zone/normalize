import { Logger } from "./logger";

export interface MathOperation {
  name: string;
  execute(a: number, b: number): number;
}

export function add(a: number, b: number): number {
  return a + b;
}

export function multiply(a: number, b: number): number {
  return a * b;
}

export class Calculator implements MathOperation {
  name: string;
  private history: number[];
  private logger: Logger;

  constructor(name: string, logger: Logger) {
    this.name = name;
    this.history = [];
    this.logger = logger;
  }

  execute(a: number, b: number): number {
    const result = add(a, b);
    this.history.push(result);
    this.logger.log(`${this.name}: ${a} + ${b} = ${result}`);
    return result;
  }

  getHistory(): number[] {
    return this.history;
  }
}
