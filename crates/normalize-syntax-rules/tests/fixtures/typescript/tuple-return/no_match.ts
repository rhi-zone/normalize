interface Pair {
    name: string;
    count: number;
}

function getPair(): Pair {
    return { name: "hello", count: 42 };
}
