import { expect, onTestFinished, test } from "vite-plus/test";

import { getDevelopmentCss, registerDevelopmentCss } from "./development.ts";

test("each project reads its own current CSS", async () => {
  let firstProjectCss = "a{color:red}";
  const disposeFirstRegistration = registerDevelopmentCss("first", async () => firstProjectCss);
  const disposeSecondRegistration = registerDevelopmentCss("second", async () => "b{color:blue}");
  onTestFinished(disposeFirstRegistration);
  onTestFinished(disposeSecondRegistration);

  expect(await getDevelopmentCss("first")).toBe("a{color:red}");
  firstProjectCss = "a{color:green}";

  expect(await getDevelopmentCss("first")).toBe("a{color:green}");
  expect(await getDevelopmentCss("second")).toBe("b{color:blue}");
});

test("disposing a registration removes its CSS getter and leaves other projects available", async () => {
  const disposeCurrentRegistration = registerDevelopmentCss("first", async () => "a{color:red}");
  const disposeSecondRegistration = registerDevelopmentCss("second", async () => "b{color:blue}");
  onTestFinished(disposeCurrentRegistration);
  onTestFinished(disposeSecondRegistration);

  disposeCurrentRegistration();

  await expect(getDevelopmentCss("first")).rejects.toThrow("unavailable");
  expect(await getDevelopmentCss("second")).toBe("b{color:blue}");
});

test("disposing a previous registration keeps the newer CSS getter under the same ID", async () => {
  const disposePreviousRegistration = registerDevelopmentCss("first", async () => "a{color:red}");
  onTestFinished(disposePreviousRegistration);

  const disposeCurrentRegistration = registerDevelopmentCss("first", async () => "a{color:purple}");
  onTestFinished(disposeCurrentRegistration);

  disposePreviousRegistration();

  expect(await getDevelopmentCss("first")).toBe("a{color:purple}");
});

test("an unknown project cannot provide CSS", async () => {
  await expect(getDevelopmentCss("unknown")).rejects.toThrow("unavailable");
});
