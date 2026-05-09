export function early_return(x: number): number {
    if (x < 0) {
        return -1;
    }
    if (x === 0) {
        return 0;
    }
    return 1;
}
