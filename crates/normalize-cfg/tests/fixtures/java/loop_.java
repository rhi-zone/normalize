class Loop {
    int loop_(int[] items) {
        int result = 0;
        for (int item : items) {
            if (item == 0) {
                break;
            }
            result += item;
        }
        return result;
    }
}
