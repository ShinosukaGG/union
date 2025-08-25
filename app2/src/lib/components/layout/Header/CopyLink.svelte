<!-- this file is human made, feel free to ignore the quality -->
<script lang="ts">
  import Button from "$lib/components/ui/Button.svelte";

  let copied = $state(false);
  let resetTimer: ReturnType<typeof setTimeout> | null = null;

  async function copyToClipboard() {
    const currentUrl = window.location.href;
    const modifiedUrl = getModifiedUrl(currentUrl, "app.union.build");

    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(modifiedUrl);
      } else {
        const ta = document.createElement("textarea");
        ta.value = modifiedUrl;
        ta.setAttribute("readonly", "");
        ta.style.position = "absolute";
        ta.style.left = "-9999px";
        document.body.appendChild(ta);
        ta.select();
        document.execCommand("copy");
        document.body.removeChild(ta);
      }

      copied = true;
      if (resetTimer) clearTimeout(resetTimer);
      resetTimer = setTimeout(() => {
        copied = false;
        resetTimer = null;
      }, 2000);
    } catch (err) {
      console.error("Failed to copy: ", err);
    }
  }

  function getModifiedUrl(currentUrl: string, host: string): string {
    try {
      const url = new URL(currentUrl);
      url.host = host;
      if (url.protocol !== "https:") url.protocol = "https:";
      url.port = "";
      return url.toString();
    } catch (e) {
      console.error("Error modifying URL:", e);
      return currentUrl;
    }
  }
</script>

<Button
  variant="icon"
  on:click={copyToClipboard}
  title="Copy link"
  aria-label="Copy link"
>
  {#if copied}
    <svg
      xmlns="http://www.w3.org/2000/svg"
      class="h-5 w-5 text-green-500"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      role="img"
      aria-hidden="true"
      focusable="false"
    >
      <polyline points="20 6 9 17 4 12"></polyline>
    </svg>
  {:else}
    <svg
      xmlns="http://www.w3.org/2000/svg"
      class="h-5 w-5"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      role="img"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path>
      <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path>
    </svg>
  {/if}
</Button>
