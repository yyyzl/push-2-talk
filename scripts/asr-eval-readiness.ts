import {
  formatReadiness,
  runAsrEvalReadinessCli,
} from "./asrEvalReadinessCore";

runAsrEvalReadinessCli(process.argv.slice(2))
  .then((result) => {
    const output = formatReadiness(result.summary, result.outputFormat);
    console.log(output.trimEnd());
    if (result.outPath) {
      console.error(
        `ASR eval readiness report written: ${result.outPath} (${output.length} bytes)`,
      );
    }
  })
  .catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    console.error(message);
    process.exitCode = 1;
  });
