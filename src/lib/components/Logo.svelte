<script lang="ts">
  let { size = 26 }: { size?: number } = $props();
</script>

<!-- The Feather wing — same artwork as the app icon (see logo.svg). -->
<svg
  width={size}
  height={size}
  viewBox="0 0 64 64"
  role="img"
  aria-label="Feather logo"
>
  <defs>
    <linearGradient id="wingman-bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#8b5cf6" />
      <stop offset="1" stop-color="#5b21b6" />
    </linearGradient>
  </defs>
  <rect x="2" y="2" width="60" height="60" rx="14" fill="url(#wingman-bg)" />
  <path
    d="M11 47 C15 28 30 15 54 12 C47 19 43 24 40 29 L47 28 C41 35 36 39 31 42 L38 42 C29 47 19 49 11 47 Z"
    fill="#ffffff"
    opacity="0.96"
  />
</svg>

<style>
  svg {
    display: block;
    border-radius: 7px;
  }
</style>
