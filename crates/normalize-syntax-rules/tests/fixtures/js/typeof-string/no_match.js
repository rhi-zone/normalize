function check(x) {
    if (typeof x === 'string') {
        return true;
    }
    if (typeof x === 'number') {
        return false;
    }
    if (typeof x !== 'undefined') {
        return null;
    }
}
