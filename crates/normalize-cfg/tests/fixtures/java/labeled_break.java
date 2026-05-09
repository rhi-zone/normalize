class LabeledBreak {
    int labeled_break(int n) {
        int result = 0;
        outer: for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                if (i == j) {
                    break outer;
                }
                if (j == 5) {
                    continue outer;
                }
                result += j;
            }
        }
        return result;
    }
}
