function checkValue(x) {
    if (x == null) {
        return "missing";
    }
    if (x != null) {
        return "present";
    }
    return "other";
}
