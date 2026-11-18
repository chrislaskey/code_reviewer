<!-- The switch for images that move, see animations.js: on, an animated
     GIF plays; off, every image shows its first frame. -->
<script>
  import { animationsOn, onAnimationsChanged, setAnimationsOn } from "./animations.js";
  import { Checkbox } from "./components/index.js";

  let on = $state(animationsOn());

  $effect(() => onAnimationsChanged((now) => (on = now)));
</script>

<Checkbox toggle size="sm" checked={on} onchange={(event) => setAnimationsOn(event.currentTarget.checked)}>
  Play animations
</Checkbox>
