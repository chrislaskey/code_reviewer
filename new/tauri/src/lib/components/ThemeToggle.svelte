<!-- System, light or dark. Dispatches the `phx:set-theme` event the theme
     script in each page's <head> listens for (src/lib/theme_script.js). -->
<script>
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  function setTheme(event) {
    event.currentTarget.dispatchEvent(
      new CustomEvent("phx:set-theme", { bubbles: true, cancelable: true, detail: { dispatcher: event.currentTarget } }),
    );
  }
</script>

<div class="card relative flex flex-row items-center border-2 border-base-300 bg-base-300 rounded-full">
  <div
    class="absolute w-1/3 h-full rounded-full border-1 border-base-200 bg-base-100 brightness-200 left-0 [[data-theme=light]_&]:left-1/3 [[data-theme=dark]_&]:left-2/3 [[data-theme-source=system]_&]:!left-0 transition-[left]"
  ></div>

  <Button class="flex p-2 cursor-pointer w-1/3" onclick={setTheme} data-phx-theme="system" title="Follow the system">
    <Icon name="hero-computer-desktop-micro" class="size-4 opacity-75 hover:opacity-100" />
  </Button>

  <Button class="flex p-2 cursor-pointer w-1/3" onclick={setTheme} data-phx-theme="light" title="Light">
    <Icon name="hero-sun-micro" class="size-4 opacity-75 hover:opacity-100" />
  </Button>

  <Button class="flex p-2 cursor-pointer w-1/3" onclick={setTheme} data-phx-theme="dark" title="Dark">
    <Icon name="hero-moon-micro" class="size-4 opacity-75 hover:opacity-100" />
  </Button>
</div>
