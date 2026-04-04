import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'defocus',
  description: 'World substrate for interactive narrative, IF, and stateful simulations',
  themeConfig: {
    nav: [
      { text: 'Guide', link: '/' },
      { text: 'rhi', link: 'https://rhi.zone/' },
    ],
    sidebar: [
      {
        text: 'defocus',
        items: [
          { text: 'Introduction', link: '/' },
        ],
      },
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/exo-place/defocus' },
    ],
  },
})
