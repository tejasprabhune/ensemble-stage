# Training runs reference

A training run is a persona fine-tuning job tracked by Stage. Stage records step-level metrics as training progresses and links the resulting adapter to subsequent eval runs, enabling a direct comparison between the trained persona and its prompted baseline.

## Creating a training run

Training runs are created by `ensemble train persona.toml`, which posts to `POST /v1/projects/{org}/{project}/training_runs` before the job starts.

```bash
ensemble train examples/agora/personas/frustrated_power_user.toml --backend modal
# Training run: https://stage.ensemble.sh/org/proj/training_runs/019b2c3d-...
```

The training pipeline streams step metrics with `POST /v1/training_runs/{id}/metrics` and finalizes with `POST /v1/training_runs/{id}/status` when the job completes, setting `final_metrics` and `artifact_uri`.

## Reading the training detail page

The training detail page at `/{org}/{project}/training_runs/{id}` answers five questions: did the loss go down, did eval improve, is this run better than past runs for this persona, did the adapter improve outcomes versus the prompted baseline, and where is the artifact.

### Header

The header shows the run ID, status badge, persona name, base model, timestamps, and, once training completes, the artifact URI with a copy button.

### Loss curves

The main visualization is one SVG chart per metric group. `train_loss` and `eval_loss` share a chart when both are present; other metrics each get their own. The x-axis is the training step; the y-axis is the metric value.

A **log scale toggle** switches between linear and log scale. Log scale is useful for loss curves that drop steeply in early steps and flatten later. The choice is stored in the URL hash so you can share a specific view.

Three automatic annotations:

- **Best eval annotation**: a dashed vertical line at the step with the lowest `eval_loss`, labeled "Best eval @ step N". This is the checkpoint most likely to generalize. If this line falls early in training, the model may have overfit in later steps.
- **Final eval marker**: a dot at the last recorded eval point, labeled with its value. A large gap between best eval and final eval indicates overfitting.
- **Plateau annotation**: a translucent band over any region where loss remained within 1% for 20% or more of total steps. A plateau indicates that further training on this configuration will not improve the result.

While training is in progress, the chart polls every 5 seconds and appends new points without redrawing.

### Hyperparameters and final metrics

Two side-by-side tables:

- **Hyperparameters**: learning rate, batch size, LoRA rank, DPO beta, epochs, and any other fields recorded at job creation.
- **Final metrics**: final train loss, final eval loss, best eval, total steps, wall time, and total cost.

Reading both together tells you what was trained and how well it converged.

### Comparison to past training runs

An overlay chart of the current run's `eval_loss` curve versus the three to five most recent completed training runs for the same persona. The current run is drawn opaque in the accent color; older runs are drawn at half opacity in a muted color.

A legend below the chart labels each line with its training run ID (first eight characters), date, and the learning rate if it differed from the current run.

Use this view to assess the effect of your hyperparameter change. If the current run's eval loss converges lower or faster, the change helped. If the lines are nearly identical, the hyperparameter does not matter much for this persona.

If this is the first training run for this persona, the section says so and will populate once additional runs exist.

### Baseline comparison

This is the most important section on the page. It answers: did the trained adapter actually improve eval outcomes compared to the prompted baseline?

Stage constructs this comparison by:

1. Finding eval runs in the same project whose `metadata.adapter_uri` matches this training run's `artifact_uri`.
2. Grouping those runs by (scenario, backend).
3. For each group, finding baseline runs in the same project on the same scenario and backend whose `metadata.persona` matches the persona name and which do not have an `adapter_uri` set.
4. Computing per-group mean scores and the delta (trained minus baseline).

The result is a table sorted by absolute delta descending:

| Column | Description |
|--------|-------------|
| Scenario | The scenario the eval runs executed. |
| Model | The backend/model used for the agent. |
| Baseline | Mean primary outcome score for prompted-persona runs. |
| Trained | Mean primary outcome score for trained-adapter runs. |
| Delta | Trained minus baseline. Green for improvement, red for regression. |
| Runs | Count in format `Nt / Nb` (trained / baseline). |

A positive delta confirms that the adapter improved outcomes on that scenario and model. A negative delta is a regression worth investigating via the comparison view for a representative run pair.

**When the comparison is empty:** The section shows an explanation:

- If no `artifact_uri` is set, the training run did not produce an artifact yet.
- If no eval runs reference the adapter, `adapter_uri` has not been recorded in run metadata. This is an integration gap: the ensemble runner must store the adapter URI when it loads a trained persona for an eval run.

### Artifact details

The artifact URI, a copy button, the artifact size if known, and two code snippets showing how to add the adapter to a persona TOML and run an eval scenario against it.

## Closing the eval loop

The baseline comparison requires five things to be in place:

1. **Training completes** with an `artifact_uri` (HuggingFace path or storage URI).
2. **The persona TOML** is updated with `adapter_name` pointing to the uploaded adapter and `serve_url` pointing to the vLLM server hosting it.
3. **A vLLM server** runs the base model with the adapter loaded: `vllm serve base-model --enable-lora --lora-modules name=hf-user/repo`.
4. **Eval runs execute** with the trained persona (mode = "trained"). The ensemble integration must record `adapter_uri` in the run's metadata pointing to the same URI as the training run's `artifact_uri`.
5. **Baseline runs execute** with the prompted persona (mode = "prompted") on the same scenarios and backends. These must have `metadata.persona` set to the persona name but no `adapter_uri`.

When all five steps are satisfied, the baseline comparison table populates automatically for any training run whose artifact was used.

## Polling behavior

Pages for runs in `running` status poll every 5 seconds for new metrics. Polling pauses when the browser tab is backgrounded and resumes when it returns to the foreground.

## API endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/projects/{org}/{proj}/training_runs` | Create training run. Body: `{persona_name, base_model, hyperparameters?}`. |
| `GET` | `/v1/projects/{org}/{proj}/training_runs` | List training runs, newest first. |
| `GET` | `/v1/training_runs/{id}` | Get run metadata including hyperparameters, final_metrics, artifact_uri. |
| `POST` | `/v1/training_runs/{id}/metrics` | Append step metrics. Body: `{metrics: [{step, metric_name, value}]}`. Up to 1000 per request. |
| `GET` | `/v1/training_runs/{id}/metrics` | All metric points in ascending step order. |
| `POST` | `/v1/training_runs/{id}/status` | Update status. Body: `{status, final_metrics?, artifact_uri?}`. |
| `GET` | `/v1/training_runs/{id}/past_runs` | Most recent completed runs for the same persona, with metrics included. |
| `GET` | `/v1/training_runs/{id}/baseline_comparison` | Trained vs. prompted comparison data. Returns `{linked, entries}`. |
