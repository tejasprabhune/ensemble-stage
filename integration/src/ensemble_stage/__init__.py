"""
ensemble-stage: push run, sweep, and training-run data from ensemble to Stage.

  from ensemble_stage import Stage

  stage = Stage()
  with stage.run(
      project="myorg/popcornbench",
      scenario="popcorn.single_problem",
      world="popcorn",
      backend="claude-sonnet-4-5",
  ) as run:
      run.append_event(sequence_number=1, kind="system", payload={"note": "started"})
"""

from .stage import Stage

__all__ = ["Stage"]
