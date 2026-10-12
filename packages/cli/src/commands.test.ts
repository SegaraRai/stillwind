import { expect, test, vi } from "vite-plus/test";

import { runCli } from "./commands.ts";

vi.mock("@stillwind/native", () => {
  throw new Error("Compiler binding unavailable");
});

test("help and structured initialization errors work when the compiler binding is unavailable", async () => {
  const stdout = vi.spyOn(process.stdout, "write").mockImplementation(() => true);
  const stderr = vi.spyOn(process.stderr, "write").mockImplementation(() => true);
  const previousExitCode = process.exitCode;
  try {
    await runCli(["--help", "--color", "never"]);
    expect(stdout.mock.calls.at(-1)?.[0]).toContain("stillwind");
    stdout.mockClear();
    await runCli(["lint", "--format", "json"]);
    expect(process.exitCode).toBe(2);
    expect(JSON.parse(String(stdout.mock.calls.at(-1)?.[0]))).toMatchObject({
      command: "lint",
      diagnostics: [],
      changed: [],
      exitCode: 2,
      failures: [{ message: expect.stringMatching(/\S/) }],
    });
    expect(stderr).not.toHaveBeenCalled();
  } finally {
    process.exitCode = previousExitCode;
    stdout.mockRestore();
    stderr.mockRestore();
  }
});
