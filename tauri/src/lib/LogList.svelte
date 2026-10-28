<script>
  import { logEntries } from "./log.svelte.js";
</script>

<section>
  <h2>Recent logs</h2>
  <ul id="log">
    {#each logEntries as entry (entry.id)}
      <li>
        {`${entry.time}  ${entry.message}`}
        {#if entry.data !== undefined}
          <pre>{JSON.stringify(entry.data, null, 2)}</pre>
        {/if}
      </li>
    {:else}
      <li class="log-placeholder">Nothing logged yet. What the buttons on this page do shows up here.</li>
    {/each}
  </ul>
</section>
