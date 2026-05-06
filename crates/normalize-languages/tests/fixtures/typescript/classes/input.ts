export class Animal {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    speak(): string {
        return `${this.name} makes a sound.`;
    }
}

export class Dog extends Animal {
    breed: string;

    constructor(name: string, breed: string) {
        super(name);
        this.breed = breed;
    }

    speak(): string {
        return `${this.name} barks.`;
    }
}

function createDog(name: string, breed: string): Dog {
    return new Dog(name, breed);
}
