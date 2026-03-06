async function processItems(items) {
    await Promise.all(items.map(item => processItem(item)));
}

async function fetchAll(urls) {
    const results = await Promise.all(urls.map(url => fetch(url)));
    return results;
}
