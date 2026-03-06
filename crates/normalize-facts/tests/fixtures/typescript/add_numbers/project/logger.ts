export interface Loggable {
  log(message: string): void;
}

export class Logger implements Loggable {
  private prefix: string;

  constructor(prefix: string) {
    this.prefix = prefix;
  }

  log(message: string): void {
    console.log(`[${this.prefix}] ${message}`);
  }
}
