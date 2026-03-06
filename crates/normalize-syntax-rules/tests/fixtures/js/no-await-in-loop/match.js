async function processItems(items) {
    for (const item of items) {
        await processItem(item);
    }
}

async function pollUntilDone(tasks) {
    while (tasks.length > 0) {
        await tasks[0]();
        tasks.shift();
    }
}
