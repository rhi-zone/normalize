import { greet } from "./utils";

export class Greeter {
    constructor(private name: string) {}

    sayHello(): string {
        return greet(this.name);
    }
}
