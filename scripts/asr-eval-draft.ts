import { runAsrEvalDraftCli } from "./asrEvalDraftCore";

runAsrEvalDraftCli(process.argv.slice(2))
  .then((result) => {
    console.log(`ASR eval draft written: ${result.outPath} (${result.count} cases)`);
  })
  .catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    console.error(message);
    process.exitCode = 1;
  });
