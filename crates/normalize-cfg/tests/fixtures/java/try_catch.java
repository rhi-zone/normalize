class TryCatch {
    int try_catch(int x) {
        try {
            if (x < 0) {
                throw new Exception("negative");
            }
            return x;
        } catch (Exception e) {
            return -1;
        } finally {
            x = 0;
        }
    }
}
