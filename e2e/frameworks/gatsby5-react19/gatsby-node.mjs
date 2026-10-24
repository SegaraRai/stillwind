import { StillwindPlugin } from "@stillwind/integration/webpack";

export function onCreateWebpackConfig({ actions, stage }) {
  actions.setWebpackConfig({ plugins: [new StillwindPlugin()] });
  if (stage === "develop") {
    // Gatsby's default matcher excludes pnpm's nested framework packages.
    actions.setWebpackConfig({
      optimization: {
        splitChunks: {
          cacheGroups: {
            framework: {
              test: /[\\/]node_modules[\\/](react|react-dom|scheduler|prop-types)[\\/]/,
            },
          },
        },
      },
    });
  }
}
