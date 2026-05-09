export function loop_(items: number[]): number {
    let result = 0;
    for (const item of items) {
        if (item === 0) {
            break;
        }
        result += item;
    }
    return result;
}
