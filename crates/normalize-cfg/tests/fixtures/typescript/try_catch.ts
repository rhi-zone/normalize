export function try_catch(x: number): number {
    try {
        if (x < 0) {
            throw new Error("negative");
        }
        return x;
    } catch (e) {
        return -1;
    } finally {
        x = 0;
    }
}
