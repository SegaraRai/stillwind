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
    const plainHelp = String(stdout.mock.calls.at(-1)?.[0]);
    expect(plainHelp).toContain("stillwind");
    expect(plainHelp).toContain("Output format (text, json, or github)");
    stdout.mockClear();

    await runCli(["--help", "--color", "always"]);
    const coloredHelp = String(stdout.mock.calls.at(-1)?.[0]);
    expect(coloredHelp).toContain("\u001b[1m");
    expect(coloredHelp).toContain("\u001b[36m");
    stdout.mockClear();

    await runCli(["unknown-cmd", "--color", "never"]);
    expect(process.exitCode).toBe(2);
    expect(String(stderr.mock.calls.at(-1)?.[0])).toContain(
      "Unknown command: unknown-cmd. Expected one of:",
    );
    stderr.mockClear();

    await runCli(["lint", "-x", "--color", "never"]);
    expect(process.exitCode).toBe(2);
    expect(String(stderr.mock.calls.at(-1)?.[0])).toContain("Unknown option: -x.");
    stderr.mockClear();

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
