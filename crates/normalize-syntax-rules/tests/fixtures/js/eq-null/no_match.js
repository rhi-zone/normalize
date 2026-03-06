function checkValue(x) {
    if (x === null) {
        return "null";
    }
    if (x === undefined) {
        return "undefined";
    }
    if (x !== null) {
        return "not null";
    }
    return "other";
}
