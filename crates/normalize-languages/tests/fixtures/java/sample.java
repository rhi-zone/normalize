import java.util.ArrayList;
import java.util.List;
import java.util.Map;

public class TaskQueue {
    private List<String> tasks;
    private int capacity;

    public TaskQueue(int capacity) {
        this.tasks = new ArrayList<>();
        this.capacity = capacity;
    }

    public boolean enqueue(String task) {
        if (tasks.size() >= capacity) {
            return false;
        }
        tasks.add(task);
        return true;
    }

    public String dequeue() {
        if (tasks.isEmpty()) {
            return null;
        }
        return tasks.remove(0);
    }

    // Returns the size
    @Override
    public int size() {
        return tasks.size();
    }

    public static String classify(int n) {
        if (n < 0) {
            return "negative";
        } else if (n == 0) {
            return "zero";
        } else {
            return "positive";
        }
    }
}

interface Processor {
    String process(String input);
}
