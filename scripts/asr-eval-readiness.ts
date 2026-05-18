import {
  formatReadinessJson,
  formatReadinessText,
  runAsrEvalReadinessCli,
} from "./asrEvalReadinessCore";

runAsrEvalReadinessCli(process.argv.slice(2))
  .then((result) => {
    const output =
      result.outputFormat === "json"
        ? formatReadinessJson(result.summary)
        : formatReadinessText(result.summary);
    console.log(output);
  })
  .catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    console.error(message);
    process.exitCode = 1;
  });
