func foo() {
    do {
        try doStuff()
    } catch {
        logger.warning("Operation failed: \(error)")
    }
}
