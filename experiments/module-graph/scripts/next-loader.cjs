const fs = require("node:fs");
const path = require("node:path");
const { createHash } = require("node:crypto");
module.exports = function (source) {
  const callback = this.async();
  const theme = path.join(this.rootContext, "theme.txt");
  this.addDependency(theme);
  this.fs.readFile(theme, "utf8", (error, value) => {
    if (error) {
      return callback(error);
    }
    const owner = path.relative(this.rootContext, this.resourcePath).replaceAll("\\", "/");
    const rules = [];
    const code = source.replace(/styleToken\('([^']+)'\)/g, (_, input) => {
      const css = input.replaceAll("THEME", String(value).trim());
      const name =
        "sw_" +
        createHash("sha256")
          .update(owner + "\0" + css)
          .digest("hex")
          .slice(0, 12);
      rules.push(`.${name}{${css}}`);
      return JSON.stringify(name);
    });
    fs.appendFileSync(
      path.join(this.rootContext, "loader-events.jsonl"),
      JSON.stringify({ owner, rules: rules.length }) + "\n",
    );
    let carrier = path
      .relative(path.dirname(this.resourcePath), path.join(this.rootContext, "carrier.css"))
      .replaceAll("\\", "/");
    if (!carrier.startsWith(".")) {
      carrier = "./" + carrier;
    }
    const request = carrier + "?sw=" + encodeURIComponent(rules.join("\n"));
    callback(null, code + (rules.length ? "\nimport " + JSON.stringify(request) + ";" : ""));
  });
};
