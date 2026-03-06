import java.util.ArrayList;
import java.util.List;

public class Calculator {
    private List<Integer> history;

    public Calculator() {
        this.history = new ArrayList<>();
    }

    public int add(int a, int b) {
        int result = a + b;
        history.add(result);
        return result;
    }

    public int multiply(int a, int b) {
        int result = a * b;
        history.add(result);
        return result;
    }

    public List<Integer> getHistory() {
        return history;
    }

    public static void main(String[] args) {
        Calculator calc = new Calculator();
        System.out.println(calc.add(2, 3));
        System.out.println(calc.multiply(4, 5));
    }
}
