fun main() {
    try {
        doStuff()
    } catch (e: Exception) {
        logger.warn("Failed", e)
    }
}
