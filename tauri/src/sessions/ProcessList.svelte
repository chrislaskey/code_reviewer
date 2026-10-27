<script>
  let { processes } = $props();
</script>

<details class="processes">
  <summary>{processes.length} {processes.length === 1 ? "process" : "processes"}</summary>
  <table>
    <tbody>
      {#each processes as process (process.pid)}
        <tr title={process.command}>
          <td>{process.pid}</td>
          <td>{process.name}</td>
          <td>{process.state}</td>
          <td>{process.in_foreground ? "foreground" : ""}</td>
          <td>{process.cpu_percent.toFixed(1)}% CPU</td>
        </tr>
      {/each}
    </tbody>
  </table>
</details>
