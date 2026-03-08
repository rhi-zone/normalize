import React, { useState, useEffect } from 'react';
import { View, Text } from 'react-native';
import type { FC } from 'react';

interface CounterProps {
    initialCount: number;
    step?: number;
    label: string;
}

interface ButtonProps {
    onClick: () => void;
    children: React.ReactNode;
}

type Theme = 'light' | 'dark';

const Button: FC<ButtonProps> = ({ onClick, children }) => (
    <button onClick={onClick}>{children}</button>
);

const Counter: FC<CounterProps> = ({ initialCount, step = 1, label }) => {
    const [count, setCount] = useState(initialCount);
    const [theme, setTheme] = useState<Theme>('light');

    useEffect(() => {
        document.title = `${label}: ${count}`;
    }, [count, label]);

    const increment = () => setCount(c => c + step);
    const decrement = () => setCount(c => c - step);
    const reset = () => setCount(initialCount);

    return (
        <div className={`counter ${theme}`}>
            <h2>{label}</h2>
            <p>Count: {count}</p>
            <Button onClick={increment}>+</Button>
            <Button onClick={decrement}>-</Button>
            <Button onClick={reset}>Reset</Button>
        </div>
    );
};

function classify(n: number): string {
    if (n < 0) {
        return 'negative';
    } else if (n === 0) {
        return 'zero';
    } else {
        return 'positive';
    }
}

export default Counter;
export { classify, Button };
