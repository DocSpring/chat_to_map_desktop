/**
 * Wait for GitHub Actions checks to complete before proceeding with release
 *
 * @param {Object} params
 * @param {Object} params.github - GitHub API object
 * @param {Object} params.context - GitHub context
 * @param {Object} params.core - GitHub Actions core
 * @param {Array<string>} params.checks - Optional array of check names to wait for
 */

const DEFAULT_REQUIRED_CHECKS = [
  'Lint & Test', // from ci.yml - main check job
  'Build' // from ci.yml - build verification job
]

const TIMEOUT_MS = 30 * 60 * 1000 // 30 minutes overall timeout
const WARMUP_MS = 5 * 60 * 1000 // 5 minutes for checks to appear
const POLL_INTERVAL_MS = 10 * 1000 // 10 seconds between polls

module.exports = async ({ github, context, core, checks }) => {
  const REQUIRED_CHECKS = checks || DEFAULT_REQUIRED_CHECKS

  if (checks) {
    core.info(`Waiting for specific checks: ${checks.join(', ')}`)
  } else {
    core.info('Waiting for all default required checks')
  }

  function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms))
  }

  async function listChecks() {
    const { data } = await github.rest.checks.listForRef({
      owner: context.repo.owner,
      repo: context.repo.repo,
      ref: context.sha,
      per_page: 100
    })
    return data.check_runs.map((c) => ({
      name: c.name,
      status: c.status, // queued, in_progress, completed
      conclusion: c.conclusion // success, failure, neutral, cancelled, etc
    }))
  }

  function matchesRequired(name) {
    return REQUIRED_CHECKS.some((check) => name === check || name.startsWith(check))
  }

  // Phase 1: Discover which required checks are actually present for this SHA
  core.info('Discovering present checks...')
  let presentChecks = []
  const warmupStart = Date.now()

  while (Date.now() - warmupStart < WARMUP_MS) {
    const allChecks = await listChecks()
    presentChecks = allChecks.filter((c) => matchesRequired(c.name))

    if (presentChecks.length > 0) {
      core.info(
        `Found ${presentChecks.length} required check(s): ${presentChecks
          .map((c) => c.name)
          .join(', ')}`
      )
      break
    }

    core.info(
      `No required checks found yet, waiting... (${Math.round(
        (Date.now() - warmupStart) / 1000
      )}s elapsed)`
    )
    await sleep(5000)
  }

  if (presentChecks.length === 0) {
    core.info('No required checks present on this commit - continuing without waiting.')
    core.info('This is normal if CI was triggered by a different event or already completed.')
    return
  }

  // Phase 2: Wait for all present checks to complete
  core.info(`Waiting for ${presentChecks.length} check(s) to complete...`)
  const waitStart = Date.now()

  while (Date.now() - waitStart < TIMEOUT_MS) {
    const allChecks = await listChecks()
    const relevant = allChecks.filter((c) => matchesRequired(c.name))

    // If checks disappeared (canceled?), keep waiting within timeout
    if (relevant.length === 0) {
      core.warning('Required checks disappeared - they may have been canceled')
      await sleep(POLL_INTERVAL_MS)
      continue
    }

    const pending = relevant.filter((c) => c.status !== 'completed')

    if (pending.length === 0) {
      // All checks completed - check their conclusions
      const failed = relevant.filter(
        (c) => c.conclusion !== 'success' && c.conclusion !== 'skipped'
      )

      if (failed.length > 0) {
        const failureDetails = failed.map((f) => `${f.name} (${f.conclusion})`).join(', ')
        core.setFailed(`Some required checks failed: ${failureDetails}`)
        process.exit(1)
      }

      const successful = relevant.filter((c) => c.conclusion === 'success')
      core.info(`✅ All ${successful.length} required check(s) passed successfully!`)
      return
    }

    // Still waiting
    const elapsed = Math.round((Date.now() - waitStart) / 1000)
    const pendingDetails = pending.map((p) => `${p.name} (${p.status})`).join(', ')
    core.info(`[${elapsed}s] Waiting for: ${pendingDetails}`)

    await sleep(POLL_INTERVAL_MS)
  }

  // Timeout reached
  core.setFailed(`Timeout after ${TIMEOUT_MS / 1000}s waiting for checks to complete`)
  process.exit(1)
}
