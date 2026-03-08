<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { writable } from 'svelte/store';
  import Counter from './Counter.svelte';

  export let title: string = 'My App';
  export let items: string[] = [];

  let count = 0;
  let filter = '';
  const store = writable(0);

  $: filtered = items.filter(item => item.includes(filter));
  $: doubled = count * 2;

  function increment() {
    count += 1;
    store.update(n => n + 1);
  }

  function reset() {
    count = 0;
    store.set(0);
  }

  onMount(() => {
    console.log('mounted');
  });

  onDestroy(() => {
    console.log('destroyed');
  });
</script>

<style>
  .container {
    max-width: 800px;
    margin: 0 auto;
    font-family: sans-serif;
  }

  h1 {
    color: #333;
  }

  .counter {
    display: flex;
    gap: 1rem;
    align-items: center;
  }

  button {
    padding: 0.5rem 1rem;
    cursor: pointer;
  }
</style>

<div class="container">
  <h1>{title}</h1>

  <div class="counter">
    <button on:click={increment}>Increment</button>
    <span>Count: {count} (doubled: {doubled})</span>
    <button on:click={reset}>Reset</button>
  </div>

  <Counter bind:value={count} />

  <input bind:value={filter} placeholder="Filter items..." />

  {#if filtered.length === 0}
    <p>No items match the filter.</p>
  {:else}
    <ul>
      {#each filtered as item, i}
        <li>{i + 1}. {item}</li>
      {/each}
    </ul>
  {/if}
</div>
