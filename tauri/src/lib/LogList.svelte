<script>
  import { logEntries } from "./log.svelte.js";
</script>

<section>
  <h2>Log</h2>
  <ul id="log">
    {#each logEntries as entry (entry.id)}
      <li>
        {`${entry.time}  ${entry.message}`}
        {#if entry.data !== undefined}
          <pre>{JSON.stringify(entry.data, null, 2)}</pre>
        {/if}
      </li>
    {/each}
  </ul>
</section>
