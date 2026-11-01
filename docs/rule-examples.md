# Rule examples

Generated from `tests/fixtures/` by `CROSGHA_BLESS=1 cargo test --test report`; never edit it
by hand. In a non-compliant example, `# expect: <rule>` marks a line the rule reports. Fixtures
also cover edge cases, so lines without a marker show what the rule must not report.

## `unpinned-package`

Tools installed in run steps must name an exact version.

**Non-compliant**

`tests/fixtures/rules/unpinned-package.before.yml`

```yaml
name: Unpinned packages
on: [push]
permissions: {}
jobs:
  tools:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: pip install requests # expect: unpinned-package
      - run: python -m pip install black==24.8.0
      - run: | # expect: unpinned-package
          pip install -r requirements.txt
          npm install -g typescript
      - run: npm install -g @scope/tool@1.2.3
      - run: go install golang.org/x/tools/cmd/goimports@latest # expect: unpinned-package
      - run: go install golang.org/x/tools/cmd/stringer@v0.24.0
      - run: cargo install cargo-deny # expect: unpinned-package
      - run: cargo install cargo-deny --version 0.16.1 --locked
      - run: cargo install --path .
      - run: gem install bundler # expect: unpinned-package
      - run: gem install bundler -v 2.5.0
      - run: pipx install ruff # expect: unpinned-package
      - run: pip install .
      - run: pip install -e .[dev]
      - run: npm install
      - run: npm ci
      - run: npm install --global --force corepack@latest # expect: unpinned-package
      - run: npm install -g typescript@next # expect: unpinned-package
      - run: npm -g install terser # expect: unpinned-package
      - run: npm -g install terser@5.36.0
      - run: go install github.com/cloudwego/thriftgo@main # expect: unpinned-package
      - run: go install github.com/cloudwego/thriftgo@3f2a1b9c
      - run: conda install -c conda-forge -y pyyaml cmake # expect: unpinned-package
      - run: mamba install -c conda-forge -y pyyaml=6.0.2 cmake==3.30.5
      - run: micromamba install --file environment-lock.txt
      - run: npm install -g some-cli@* # expect: unpinned-package
      - run: npm install -g some-cli@^1.2.0 # expect: unpinned-package
      - run: pnpm add -g some-cli@~1.2.0 # expect: unpinned-package
      - run: npm install some-cli@1.x # expect: unpinned-package
      - run: npm install some-cli@1 # expect: unpinned-package
      - run: npm install 'some-cli@>=1.0.0 <2' # expect: unpinned-package
      - run: npm install some-cli@1.2.3-beta.1
      - run: npm install "typescript@5.4.5" 'eslint@9.9.0'
      - run: npm install "typescript@5" # expect: unpinned-package
      - run: npm install --workspace packages/app lodash@4.17.21
      - run: npm install -w packages/app --save-dev lodash # expect: unpinned-package
      - run: pnpm install --loglevel warn --frozen-lockfile
      - run: npm install --omit dev --include optional
      - run: npm install --tag beta
      - run: pnpm install --filter hangar
      - run: npm install ../pkg/${{ steps.pack.outputs.tarball }}
      - run: npm install -g ${{ inputs.tool }} prettier # expect: unpinned-package
      - run: npm install -g ${{ inputs.tool || 'prettier' }}@3.3.3
      - parallel:
          - run: npm install eslint # expect: unpinned-package
          - run: npm install eslint@9.9.0
      - run: cargo install --version=0.28.0 cargo-dist --locked
      - run: cargo install --vers=0.28.0 cargo-dist
      - run: cargo install --git=https://example.com/tool.git tool
      - run: gem install bundler --version=2.4.22
      - run: gem install bundler -v2.4.22
      - run: uv tool install ruff@0.6.0
      - run: uv tool install ruff==0.6.0
      - run: uv tool install ruff@latest # expect: unpinned-package
      - run: uv tool install ruff # expect: unpinned-package
      - run: go install github.com/golangci/golangci-lint/cmd/golangci-lint@v1 # expect: unpinned-package
      - run: go install golang.org/x/tools/gopls@v0.16 # expect: unpinned-package
      - run: go install example.com/tool@v1.2.3-rc.1
      - run: go install example.com/tool@v0.0.0-20240101000000-abcdefabcdef
      - run: go install example.com/tool@v2.0.0+incompatible
      - run: npm --prefix ./tools install eslint # expect: unpinned-package
      - run: npm --prefix ./tools install eslint@9.9.0
      - run: npm --prefix=./tools install eslint # expect: unpinned-package
      - run: pnpm --dir ./tools add eslint # expect: unpinned-package
      - run: pnpm -C ./tools add eslint # expect: unpinned-package
```

**Compliant**

`tests/fixtures/rules/unpinned-package.clean.yml`

```yaml
name: Unpinned packages
on: [push]
permissions: {}
jobs:
  tools:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: pip install requests==2.32.3
      - run: python -m pip install black==24.8.0
      - run: |
          pip install -r requirements.txt
          npm install -g typescript@5.6.2
      - run: npm install -g @scope/tool@1.2.3
      - run: go install golang.org/x/tools/cmd/goimports@v0.24.0
      - run: go install golang.org/x/tools/cmd/stringer@v0.24.0
      - run: cargo install cargo-deny@0.16.1 --locked
      - run: cargo install cargo-deny --version 0.16.1 --locked
      - run: cargo install --path .
      - run: gem install bundler:2.5.0
      - run: gem install bundler -v 2.5.0
      - run: pipx install ruff==0.6.9
      - run: pip install .
      - run: pip install -e .[dev]
      - run: npm install
      - run: npm ci
      - run: npm install --global --force corepack@0.30.0
      - run: npm install -g typescript@5.6.2
      - run: npm -g install terser@5.36.0
      - run: npm -g install terser@5.36.0
      - run: go install github.com/cloudwego/thriftgo@v0.3.18
      - run: go install github.com/cloudwego/thriftgo@3f2a1b9c
      - run: conda install -c conda-forge -y pyyaml=6.0.2 cmake=3.30.5
      - run: mamba install -c conda-forge -y pyyaml=6.0.2 cmake==3.30.5
      - run: micromamba install --file environment-lock.txt
      - run: npm install -g some-cli@1.2.3
      - run: npm install -g some-cli@1.2.3
      - run: pnpm add -g some-cli@1.2.3
      - run: npm install some-cli@1.2.3
      - run: npm install some-cli@1.2.3
      - run: npm install some-cli@1.2.3
      - run: npm install some-cli@1.2.3-beta.1
      - run: npm install "typescript@5.4.5" 'eslint@9.9.0'
      - run: npm install "typescript@5.4.5"
      - run: npm install --workspace packages/app lodash@4.17.21
      - run: npm install -w packages/app --save-dev lodash@4.17.21
      - run: pnpm install --loglevel warn --frozen-lockfile
      - run: npm install --omit dev --include optional
      - run: npm install --tag beta
      - run: pnpm install --filter hangar
      - run: npm install ../pkg/${{ steps.pack.outputs.tarball }}
      - run: npm install -g ${{ inputs.tool }} prettier@3.3.3
      - run: npm install -g ${{ inputs.tool || 'prettier' }}@3.3.3
      - parallel:
          - run: npm install eslint@9.9.0
          - run: npm install eslint@9.9.0
```

## `unverified-remote-script`

Downloaded scripts must not run before their integrity is checked.

**Non-compliant**

`tests/fixtures/rules/unverified-remote-script.before.yml`

```yaml
name: Remote scripts
on: [push]
permissions: {}
jobs:
  setup:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: curl -fsSL https://example.com/install.sh | sh # expect: unverified-remote-script
      - run: | # expect: unverified-remote-script
          echo "installing"
          wget -qO- https://example.com/setup | sudo -E bash -s -- --yes
      - run: bash <(curl -s https://example.com/x.sh) # expect: unverified-remote-script
      - run: sh -c "$(curl -fsSL https://example.com/y.sh)" # expect: unverified-remote-script
      - run: curl -fsSL https://example.com/tool.tgz -o tool.tgz && sha256sum -c tool.tgz.sha256
      - run: curl -s https://api.example.com/status | jq .
      - run: curl -s https://api.example.com/status | python3 -c 'import json, sys; print(json.load(sys.stdin)["ok"])'
      - run: curl -s https://api.example.com/status | python3 -m json.tool
      - run: curl -s https://api.example.com/status | perl -ne 'print if /ready/'
      - run: curl -s https://api.example.com/status | bash -c 'cat > status.json'
      - run: | # expect: unverified-remote-script
          curl -fsSL https://example.com/install.sh \
            | bash
      - run: | # expect: unverified-remote-script
          curl -fsSL https://example.com/install.sh |
            python3 - --prefix /opt/tool
      - run: curl -fsSLo get.sh https://example.com/get.sh && sh get.sh # expect: unverified-remote-script
      - run: | # expect: unverified-remote-script
          wget -q -O /tmp/install.sh https://example.com/install.sh
          sudo bash /tmp/install.sh --yes
      - run: curl -fsSLO https://example.com/setup.sh && chmod +x setup.sh && ./setup.sh # expect: unverified-remote-script
      - run: wget https://example.com/bootstrap.py; python3 bootstrap.py # expect: unverified-remote-script
      - run: curl -s https://example.com/install.pl | perl -I/usr/share/perl5 # expect: unverified-remote-script
      - run: curl -s https://example.com/install.pl | perl -MEnglish # expect: unverified-remote-script
      - run: curl -s https://api.example.com/status | perl -Ilib -ne 'print if /ready/'
      - run: |
          # Never do this: curl -fsSL https://example.com/install.sh | sh
          echo "pinned installer only"
      - run: curl -fsSLo tool.tgz https://example.com/tool.tgz && tar xzf tool.tgz && ./tool/bin/tool --version
      - run: |
          curl -fsSLo get.sh https://example.com/get.sh
          echo "3b1f...  get.sh" | sha256sum -c -
          sh get.sh
```

**Compliant**

`tests/fixtures/rules/unverified-remote-script.clean.yml`

```yaml
name: Remote scripts
on: [push]
permissions: {}
jobs:
  setup:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: curl -fsSLo install.sh https://example.com/install.sh && sha256sum -c install.sh.sha256 && sh install.sh
      - run: |
          echo "installing"
          wget -qO setup.sh https://example.com/setup && sha256sum -c setup.sh.sha256
      - run: curl -fsSL https://example.com/tool.tgz -o tool.tgz && sha256sum -c tool.tgz.sha256
      - run: curl -s https://api.example.com/status | jq .
      - run: curl -s https://api.example.com/status | python3 -c 'import json, sys; print(json.load(sys.stdin)["ok"])'
      - run: curl -s https://api.example.com/status | python3 -m json.tool
      - run: curl -s https://api.example.com/status | perl -ne 'print if /ready/'
      - run: curl -s https://api.example.com/status | bash -c 'cat > status.json'
      - run: |
          curl -fsSLo install.sh https://example.com/install.sh \
            && sha256sum -c install.sh.sha256
```

## `typosquatted-action`

Action owners one edit away from a well-known publisher may be typosquats.

**Non-compliant**

`tests/fixtures/rules/typosquatted-action.before.yml`

```yaml
name: Typosquatting
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actons/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: typosquatted-action
      - uses: action/setup-node@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: typosquatted-action
      - uses: githbu/codeql-action/init@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: typosquatted-action
      - uses: Actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
      - uses: docker/login-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: acme/actions@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: actionsx/prettier@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  reuse:
    uses: dokcer/workflows/.github/workflows/build.yml@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: typosquatted-action
```

**Compliant**

`tests/fixtures/rules/typosquatted-action.clean.yml`

```yaml
name: Typosquatting
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: actions/setup-node@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: github/codeql-action/init@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: Actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: docker/login-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: acme/actions@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  reuse:
    uses: docker/workflows/.github/workflows/build.yml@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

## `script-injection`

Untrusted event data must not be expanded into scripts with `${{ }}`.

**Non-compliant**

`tests/fixtures/rules/script-injection.before.yml`

```yaml
name: Script injection
on: [issues, pull_request]
permissions: {}
jobs:
  triage:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo "${{ github.event.issue.title }}" # expect: script-injection
      - run: | # expect: script-injection
          echo "branch ${{ github.head_ref }}"
      - run: echo "${{ github.event.issue.number }}"
      - env:
          TITLE: ${{ github.event.issue.title }}
        run: echo "$TITLE"
      - uses: actions/github-script@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          script: | # expect: script-injection
            console.log("${{ github.event.pull_request.body }}")
      - run: echo "${{ github.event.commits[0].message }}" # expect: script-injection
      - run: echo "${{ github.event.pull_request.head.repo.default_branch }}" # expect: script-injection
      - run: echo "${{ github.sha }} ${{ github.ref }}"
      # Index syntax, upper case, and whole objects read the same untrusted text.
      - run: echo "${{ github.event.issue['title'] }}" # expect: script-injection
      - run: echo "${{ github.event.issue.TITLE }}" # expect: script-injection
      - run: echo '${{ toJSON(github.event.issue) }}' | jq .labels # expect: script-injection
      - run: echo '${{ toJSON(github) }}' # expect: script-injection
      - run: echo "${{ github.event.workflow_run.head_branch }}" # expect: script-injection
      # The run's title is the triggering commit message or pull request title; `changes` holds
      # the text an `edited` event replaced.
      - run: echo "${{ github.event.workflow_run.display_title }}" # expect: script-injection
      - run: echo "${{ github.event.changes.title.from }}" # expect: script-injection
      - run: echo "${{ github.event.changes.body.from }}" # expect: script-injection
      - run: echo "${{ github.event.changes.base.sha.from }}"
      # Spaces inside the index and `}}` inside a string literal do not end the path or expression.
      - run: echo "${{ github.event[ 'issue' ].title }}" # expect: script-injection
      - run: echo "${{ format('}}{0}', github.event.issue.title) }}" # expect: script-injection
      - run: echo "${{ format('it''s }}{0}', github.event.issue.number) }}"
      - run: echo "${{ github.event.issue }} ${{ inputs.github_token }}"
      - run: echo '${{ toJSON(github.event.sender) }}'
      # A scalar leaf serializes to a number or hash; a nested object still carries its text.
      - run: echo '${{ toJSON(github.event.issue.number) }} ${{ toJSON(github.event.pull_request.head.sha) }}'
      - run: echo '${{ toJSON(github.event.pull_request.created_at) }} ${{ toJSON(github.event.issue.html_url) }}'
      - run: echo '${{ toJSON(github.event.pull_request.head) }}' # expect: script-injection
      - run: echo '${{ toJSON(github.event.commits[0]) }}' # expect: script-injection
      - uses: example/action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          title: ${{ github.event.issue.title }}
      # A comparison, `!`, or boolean function returns only true or false, so no text reaches the script.
      - run: |
          if [ "${{ startsWith(github.event.comment.body, '/deploy') }}" = "true" ]; then echo deploy; fi
      - run: echo "is-bug=${{ contains(github.event.issue.labels.*.name, 'bug') }}" >> "$GITHUB_OUTPUT"
      - run: echo "${{ endsWith(github.event.issue.title, '!') }}"
      - run: echo "${{ github.event.issue.body == '' }}"
      - run: echo "${{ github.event.issue.title != 'a && b' }}"
      - run: echo "${{ !github.event.issue.body }}"
      - run: echo "${{ (github.event.issue.title == 'x') }}"
      - run: echo "${{ contains(github.event.issue.title, 'x') && github.event.issue.body == '' }}"
      # `&&` and `||` return an operand, and these functions return text.
      - run: echo "${{ github.event.issue.body == '' || github.event.issue.body }}" # expect: script-injection
      - run: echo "${{ contains(github.event.issue.title, 'x') && github.event.issue.title }}" # expect: script-injection
      - run: echo "${{ format('{0}', github.event.comment.body) }}" # expect: script-injection
      - run: echo "${{ join(github.event.issue.labels.*.name) }}" # expect: script-injection
      - run: echo "${{ (github.event.issue.title) }}" # expect: script-injection
      - run: echo "${{ format('{0}', github.event.issue.title == 'x') }} ${{ github.event.issue.title }}" # expect: script-injection
```

**Compliant**

`tests/fixtures/rules/script-injection.clean.yml`

```yaml
name: Script injection
on: [issues, pull_request]
permissions: {}
jobs:
  triage:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - env:
          ISSUE_TITLE: ${{ github.event.issue.title }}
        run: echo "$ISSUE_TITLE"
      - env:
          HEAD_REF: ${{ github.head_ref }}
        run: |
          echo "branch $HEAD_REF"
      - run: echo "${{ github.event.issue.number }}"
      - uses: actions/github-script@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        env:
          BODY: ${{ github.event.pull_request.body }}
        with:
          script: |
            console.log(process.env.BODY)
      - run: echo "${{ github.sha }} ${{ github.ref }}"
      - env:
          ISSUE: ${{ toJSON(github.event.issue) }}
        run: echo "$ISSUE" | jq .labels
```

## `github-env-injection`

GITHUB_ENV and GITHUB_PATH must not be written from untrusted data or in privileged runs.

**Non-compliant**

`tests/fixtures/rules/github-env-injection.before.yml`

```yaml
name: Environment file writes
on:
  pull_request_target:
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo "NAME=value" >> "$GITHUB_ENV" # expect: github-env-injection
      - run: | # expect: github-env-injection
          echo "$HOME/bin" >> $GITHUB_PATH
      - run: echo "out=1" >> "$GITHUB_OUTPUT"
      # Reading the paths writes nothing.
      - run: |
          echo "$GITHUB_ENV" > debug.txt
          cat "$GITHUB_PATH" | tee path.txt
```

**Compliant**

`tests/fixtures/rules/github-env-injection.clean.yml`

```yaml
name: Environment file writes
on:
  pull_request_target:
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    env:
      NAME: value
    steps:
      - run: echo "$HOME/bin/tool"
      - run: echo "out=1" >> "$GITHUB_OUTPUT"
      - run: echo "$GITHUB_ENV" > debug.txt
```

## `llm-prompt-injection`

AI agent actions must not receive untrusted event data or run on events outsiders trigger.

**Non-compliant**

`tests/fixtures/rules/llm-prompt-injection.before.yml`

```yaml
name: Agent on comments
on:
  issue_comment:
    types: [created]
permissions: {}
jobs:
  agent:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: anthropics/claude-code-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: llm-prompt-injection
        with:
          prompt: Review this issue
      - uses: example/labeler@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

**Compliant**

`tests/fixtures/rules/llm-prompt-injection.clean.yml`

```yaml
name: Agent on comments
on:
  issue_comment:
    types: [created]
permissions: {}
jobs:
  agent:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: example/labeler@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

## `pwn-request`

Privileged triggers must not check out or run pull request code.

**Non-compliant**

`tests/fixtures/rules/pwn-request.before.yml`

```yaml
name: Privileged PR build
on:
  pull_request_target:
    types: [opened]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: pwn-request
        with:
          ref: ${{ github.event.pull_request.head.sha }}
          persist-credentials: false
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          persist-credentials: false
      - run: gh pr checkout ${{ github.event.pull_request.number }} # expect: pwn-request
      - run: | # expect: pwn-request
          git fetch origin pull/${{ github.event.number }}/head:pr
          git checkout pr
      # The fork's default branch is the author's code even without a `ref`.
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: pwn-request
        with:
          repository: ${{ github.event.pull_request.head.repo.full_name }}
          persist-credentials: false
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: pwn-request
        with:
          ref: ${{ github.event.pull_request.merge_commit_sha }}
          persist-credentials: false
      - run: | # expect: pwn-request
          git fetch --depth 1 \
            origin "+refs/pull/${{ github.event.number }}/merge"
      - run: | # expect: pwn-request
          git remote add fork "https://github.com/${{ github.event.pull_request.head.repo.full_name }}"
          git fetch fork "${{ github.event.pull_request.head.sha }}" && git switch --detach FETCH_HEAD
      # Index syntax and upper case name the same head.
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: pwn-request
        with:
          ref: ${{ github.event.pull_request['head']['sha'] }}
          persist-credentials: false
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: pwn-request
        with:
          ref: ${{ github.event.pull_request.HEAD.sha }}
          persist-credentials: false
      - run: git fetch origin "${{ github.event.pull_request.head['ref'] }}" # expect: pwn-request
      # Global options before the subcommand, `pull`, and `clone` reach the same code.
      - run: git -C src fetch origin pull/1/head # expect: pwn-request
      - run: git -c protocol.version=2 fetch origin refs/pull/1/merge # expect: pwn-request
      - run: | # expect: pwn-request
          cd src && git --git-dir=.git --work-tree . pull \
            https://github.com/${{ github.event.pull_request.head.repo.full_name }} ${{ github.head_ref }}
      - run: git clone ${{ github.event.pull_request.head.repo.clone_url }} pr # expect: pwn-request
      - run: sudo -u runner /usr/bin/git fetch origin pull/1/head # expect: pwn-request
      # Fetching the base branch and naming the pull request elsewhere is not a checkout.
      - run: |
          git -C pull/ fetch origin main
          git commit -m "Apply review of ${{ github.head_ref }}"
          echo "git fetch origin pull/${{ github.event.number }}/head"
          git fetch origin main
          curl -fsS "https://api.github.com/repos/${{ github.repository }}/pulls/${{ github.event.number }}"
          echo "see https://github.com/${{ github.repository }}/pull/${{ github.event.number }}"
```

**Compliant**

`tests/fixtures/rules/pwn-request.clean.yml`

```yaml
name: Privileged PR build
on:
  pull_request_target:
    types: [opened]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          persist-credentials: false
      - run: gh pr view ${{ github.event.pull_request.number }} --json title
      - run: |
          git fetch origin main
          echo "see https://github.com/${{ github.repository }}/pull/${{ github.event.number }}"
```

## `dangerous-trigger`

pull_request_target and workflow_run run with base repository privileges.

**Non-compliant**

`tests/fixtures/rules/dangerous-trigger.before.yml`

```yaml
name: Triggers
on:
  push:
    branches: [main]
  pull_request_target: # expect: dangerous-trigger
  workflow_run: # expect: dangerous-trigger
    workflows: [CI]
permissions: {}
jobs:
  noop:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo ok
```

**Compliant**

`tests/fixtures/rules/dangerous-trigger.clean.yml`

```yaml
name: Triggers
on:
  push:
    branches: [main]
  pull_request:
permissions: {}
jobs:
  noop:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo ok
```

## `unsafe-if-condition`

Conditions must not be constant, always true, or based on spoofable bot names.

**Non-compliant**

`tests/fixtures/rules/unsafe-if-condition.before.yml`

```yaml
name: Conditions
on: [pull_request]
permissions: {}
jobs:
  test:
    if: true # expect: unsafe-if-condition
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - if: ${{ github.event_name == 'push' }} && ${{ github.ref == 'refs/heads/main' }} # expect: unsafe-if-condition
        run: echo a
      - if: | # expect: unsafe-if-condition
          ${{ github.event_name == 'push' }}
        run: echo b
      - if: github.actor == 'dependabot[bot]' # expect: unsafe-if-condition
        run: echo c
      - if: github.event_name == 'push'
        run: echo d
      - if: ${{ github.event_name == 'push' }}
        run: echo e
      - if: github.event.pull_request.user.login == 'dependabot[bot]'
        run: echo f
      - if: ${{ 'Dependabot[bot]' == GitHub.Actor }} # expect: unsafe-if-condition
        run: echo g
      - if: contains(fromJSON('["dependabot[bot]", "renovate[bot]"]'), github.triggering_actor) # expect: unsafe-if-condition
        run: echo h
      - if: github.event_name == 'pull_request' && endsWith(github.actor, '[bot]') # expect: unsafe-if-condition
        run: echo i
      # A spoofed bot name fails these checks, so it can only skip the step.
      - if: github.actor != 'dependabot[bot]'
        run: echo j
      - if: ${{ !contains(fromJSON('["dependabot[bot]", "renovate[bot]"]'), github.actor) }}
        run: echo k
      - if: github.actor == 'octocat' && github.event.pull_request.user.login == 'dependabot[bot]'
        run: echo l
      - if: ${{ !(github.actor == 'dependabot[bot]' || github.actor == 'renovate[bot]') }}
        run: echo m
      - if: "!(contains(fromJSON('[\"dependabot[bot]\"]'), github.actor))"
        run: echo n
      - if: contains(github.actor, '[bot]') == false
        run: echo o
      # The negation covers another check, or a second one undoes it.
      - if: ${{ !(github.event_name == 'push') && github.actor == 'dependabot[bot]' }} # expect: unsafe-if-condition
        run: echo p
      - if: ${{ !(endsWith(github.actor, '[bot]') == false) }} # expect: unsafe-if-condition
        run: echo q
```

**Compliant**

`tests/fixtures/rules/unsafe-if-condition.clean.yml`

```yaml
name: Conditions
on: [pull_request]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - if: github.event_name == 'push' && github.ref == 'refs/heads/main'
        run: echo a
      - if: ${{ github.event_name == 'push' }}
        run: echo b
      - if: github.event.pull_request.user.login == 'dependabot[bot]'
        run: echo c
      - if: github.event_name == 'push'
        run: echo d
      - if: github.actor != 'dependabot[bot]'
        run: echo e
```

## `artifact-poisoning`

Artifacts from other runs must not be extracted into the workspace.

**Non-compliant**

`tests/fixtures/rules/artifact-poisoning.before.yml`

```yaml
name: Report
on:
  workflow_run:
    workflows: [CI]
    types: [completed]
permissions: {}
jobs:
  report:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: artifact-poisoning
        with:
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ github.token }}
      - uses: dawidd6/action-download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: artifact-poisoning
        with:
          workflow: ci.yml
      - uses: actions/download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ github.token }}
          path: ${{ runner.temp }}/artifacts
      - uses: actions/download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          name: same-run
```

**Compliant**

`tests/fixtures/rules/artifact-poisoning.clean.yml`

```yaml
name: Report
on:
  workflow_run:
    workflows: [CI]
    types: [completed]
permissions: {}
jobs:
  report:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ github.token }}
          path: ${{ runner.temp }}/artifacts
      - uses: dawidd6/action-download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          workflow: ci.yml
          path: /tmp/artifacts
```

## `cache-poisoning`

Caches must not be writable from low-trust triggers or restored into release builds.

**Non-compliant**

`tests/fixtures/rules/cache-poisoning.before.yml`

```yaml
name: Release
on:
  push:
    tags: ['v*']
  release:
    types: [published]
permissions: {}
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/cache@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: cache-poisoning
        with:
          path: ~/.cargo
          key: cargo
      - uses: actions/setup-node@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: cache-poisoning
        with:
          cache: npm
      - uses: actions/setup-go@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: cache-poisoning
      - uses: actions/setup-go@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          cache: false
      - uses: actions/setup-node@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      # setup-go's `cache` input defaults to false before v4.
      - uses: actions/setup-go@v3
      - uses: actions/setup-go@v3.6.1 # expect: cache-poisoning
        with:
          cache: true
      - uses: actions/setup-go@v4 # expect: cache-poisoning
      # setup-uv caches by default from v5; from v10 its `auto` default skips release and tag runs.
      - uses: astral-sh/setup-uv@v4
      - uses: astral-sh/setup-uv@v7 # expect: cache-poisoning
      - uses: astral-sh/setup-uv@v10
      - uses: astral-sh/setup-uv@v10 # expect: cache-poisoning
        with:
          enable-cache: true
  publish:
    uses: ./.github/workflows/publish.yml
```

**Compliant**

`tests/fixtures/rules/cache-poisoning.clean.yml`

```yaml
name: Release
on:
  push:
    tags: ['v*']
permissions: {}
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/setup-node@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: actions/setup-go@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          cache: false
```

## `obfuscation`

`uses` paths and expressions must not hide what they reference.

**Non-compliant**

`tests/fixtures/rules/obfuscation.before.yml`

```yaml
name: Obfuscation
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions//checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: obfuscation
      - uses: actions/cache/save/../restore@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: obfuscation
      - uses: github/codeql-action/./init@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: obfuscation
      - run: echo ${{ 'hello' }} # expect: obfuscation
      - run: echo ${{ github['event']['issue']['title'] }} # expect: obfuscation
      - uses: github/codeql-action/init@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: docker://ghcr.io/danger/danger-kotlin:1.2.0
      - run: echo ${{ github.event.issue.number }} ${{ matrix['os'] }}
```

**Compliant**

`tests/fixtures/rules/obfuscation.clean.yml`

```yaml
name: Obfuscation
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: actions/cache/restore@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: github/codeql-action/init@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - run: echo hello
      - run: echo ${{ github.event.issue.number }} ${{ matrix['os'] }}
```

## `excessive-permissions`

GITHUB_TOKEN must not get read-all, write-all, or workflow-level write scopes that several jobs inherit.

**Non-compliant**

`tests/fixtures/rules/excessive-permissions.before.yml`

```yaml
name: Permissions
on: [push]
permissions: write-all # expect: excessive-permissions
jobs:
  a:
    permissions: write-all # expect: excessive-permissions
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo a
  b:
    permissions:
      contents: write
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo b
```

**Compliant**

`tests/fixtures/rules/excessive-permissions.clean.yml`

```yaml
name: Permissions
on: [push]
permissions: {}
jobs:
  a:
    permissions:
      contents: read
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo a
  b:
    permissions:
      contents: write
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo b
```

## `missing-permissions-block`

Jobs must declare GITHUB_TOKEN permissions instead of relying on defaults.

**Non-compliant**

`tests/fixtures/rules/missing-permissions-block.before.yml`

```yaml
name: Token permissions
on: [push]
jobs:
  build: # expect: missing-permissions-block
    runs-on: ubuntu-22.04
    timeout-minutes: 10
    steps:
      - run: cargo test
  publish:
    runs-on: ubuntu-22.04
    timeout-minutes: 10
    permissions:
      contents: write
    steps:
      - run: echo 'Publish fixture'
  reuse: # expect: missing-permissions-block
    uses: ./.github/workflows/reusable.yml
```

**Compliant**

`tests/fixtures/rules/missing-permissions-block.clean.yml`

```yaml
name: Token permissions
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-22.04
    timeout-minutes: 10
    steps:
      - run: cargo test
  publish:
    runs-on: ubuntu-22.04
    timeout-minutes: 10
    permissions:
      contents: write
    steps:
      - run: echo 'Publish fixture'
  reuse:
    uses: ./.github/workflows/reusable.yml
```

## `secrets-exposure`

Secrets must not be exposed to every step or read in bulk.

**Non-compliant**

`tests/fixtures/rules/secrets-exposure.before.yml`

```yaml
name: Secrets
on: [push]
permissions: {}
env:
  TOKEN: ${{ secrets.DEPLOY_TOKEN }} # expect: secrets-exposure
  T2: ${{ SECRETS.OTHER }} # expect: secrets-exposure
  T3: ${{ secrets['THIRD'] }} # expect: secrets-exposure
  T4: ${{ secrets [ 'FOURTH' ] }} # expect: secrets-exposure
  # Text that only looks like the context, in a literal or a longer name.
  NOT_SECRET: ${{ 'secrets.NOPE' }}
  NOT_SECRET_2: ${{ env.my_secrets.value }}
  MODE: production
jobs:
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo '${{ toJSON(secrets) }}' > secrets.json # expect: secrets-exposure
      - env:
          VALUE: ${{ secrets[format('TOKEN_{0}', github.ref_name)] }} # expect: secrets-exposure
        run: ./deploy
      - env:
          TOKEN: ${{ secrets.DEPLOY_TOKEN }}
        run: ./deploy
```

**Compliant**

`tests/fixtures/rules/secrets-exposure.clean.yml`

```yaml
name: Secrets
on: [push]
permissions: {}
env:
  MODE: production
jobs:
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - env:
          TOKEN: ${{ secrets.DEPLOY_TOKEN }}
        run: ./deploy
```

## `secrets-inherit`

Reusable workflow calls should pass only the secrets they need.

**Non-compliant**

`tests/fixtures/rules/secrets-inherit.before.yml`

```yaml
name: Reusable workflow secrets
on: [workflow_dispatch]
permissions: { contents: read }
jobs:
  local:
    uses: ./.github/workflows/build.yml
    secrets: inherit # expect: secrets-inherit
  remote:
    uses: example/workflows/.github/workflows/deploy.yml@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    secrets: inherit # expect: secrets-inherit
  selected:
    uses: example/workflows/.github/workflows/deploy.yml@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
    secrets:
      deploy-token: ${{ secrets.DEPLOY_TOKEN }}
  none:
    uses: ./.github/workflows/build.yml
```

**Compliant**

`tests/fixtures/rules/secrets-inherit.clean.yml`

```yaml
name: Reusable workflow secrets
on: [workflow_dispatch]
permissions: { contents: read }
jobs:
  local:
    uses: ./.github/workflows/build.yml
  remote:
    uses: example/workflows/.github/workflows/deploy.yml@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    secrets:
      deploy-token: ${{ secrets.DEPLOY_TOKEN }}
  selected:
    uses: example/workflows/.github/workflows/deploy.yml@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
    secrets:
      deploy-token: ${{ secrets.DEPLOY_TOKEN }}
  none:
    uses: ./.github/workflows/build.yml
```

## `hardcoded-credentials`

Passwords and access tokens must not be written in workflows.

**Non-compliant**

`tests/fixtures/rules/hardcoded-credentials.before.yml`

```yaml
name: Credentials
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    container:
      image: ghcr.io/example/app:1
      credentials:
        username: ci
        password: hunter2 # expect: hardcoded-credentials
    services:
      db:
        image: postgres:16
        credentials:
          username: ci
          password: ${{ secrets.DB_PASSWORD }}
    steps:
      - run: | # expect: hardcoded-credentials
          curl -H "Authorization: token ghp_000000000000000000000000000000000000" https://api.github.com
      - env:
          AWS_ACCESS_KEY_ID: AKIAIOSFODNN7EXAMPLE # expect: hardcoded-credentials
          REGION: us-east-1
        run: ./deploy
```

**Compliant**

`tests/fixtures/rules/hardcoded-credentials.clean.yml`

```yaml
name: Credentials
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    container:
      image: ghcr.io/example/app:1
      credentials:
        username: ci
        password: ${{ secrets.REGISTRY_PASSWORD }}
    steps:
      - env:
          TOKEN: ${{ secrets.API_TOKEN }}
        run: |
          curl -H "Authorization: token $TOKEN" https://api.github.com
      - env:
          AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
          REGION: us-east-1
        run: ./deploy
```

## `persist-credentials`

actions/checkout must not leave the token on disk for later steps; there is no automatic fix when a later step pushes with it, `true` is explicit, or the checkout predates v2.

**Non-compliant**

`tests/fixtures/rules/persist-credentials.before.yml`

```yaml
name: Checkout credentials
on: [push]
permissions: { contents: read }
jobs:
  test:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0 # expect: persist-credentials
      - name: Build
        run: cargo test
  package:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - name: Checkout for packaging
        uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0 # expect: persist-credentials
        with:
          fetch-depth: 0 # Keep tags for release notes.
      - uses: actions/cache@9a2ac9cf5c153b7fae9bf08707503731d4c640be # v4.2.0
        with:
          path: target
          key: build-cache
      - name: Checkout release notes
        uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0 # expect: persist-credentials
        with:
          path: release-notes
  already-safe:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with: { persist-credentials: false }
  mixed-case:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      # GitHub resolves action names case-insensitively.
      - uses: Actions/Checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0 # expect: persist-credentials
      - run: make docs
  tag-notes:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0 # expect: persist-credentials
      - name: Print the tag command
        run: echo "Tag ready; publish it with git push origin v1.2.3"
      - run: git log -1 --format=%H
```

**Compliant**

`tests/fixtures/rules/persist-credentials.clean.yml`

```yaml
name: Checkout credentials
on: [push]
permissions: { contents: read }
jobs:
  test:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with:
          persist-credentials: false
      - name: Build
        run: cargo test
  package:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - name: Checkout for packaging
        uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with:
          fetch-depth: 0 # Keep tags for release notes.
          persist-credentials: false # No push is needed here.
      - uses: actions/cache@9a2ac9cf5c153b7fae9bf08707503731d4c640be # v4.2.0
        with:
          path: target
          key: build-cache
      - name: Checkout release notes
        uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with:
          path: release-notes
          persist-credentials: false
  already-safe:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with: { persist-credentials: false }
  mixed-case:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: Actions/Checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with:
          persist-credentials: false
      - run: make docs
  tag-notes:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with:
          persist-credentials: false
      - name: Print the tag command
        run: echo "Tag ready; publish it with git push origin v1.2.3"
      - run: git log -1 --format=%H
```

## `oidc-misconfig`

`id-token: write` must be scoped to the job that needs it.

**Non-compliant**

`tests/fixtures/rules/oidc-misconfig.before.yml`

```yaml
name: OIDC
on: [push]
permissions:
  contents: read
  id-token: write # expect: oidc-misconfig
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo test
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo deploy
```

**Compliant**

`tests/fixtures/rules/oidc-misconfig.clean.yml`

```yaml
name: OIDC
on: [push]
permissions:
  contents: read
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo test
  deploy:
    permissions:
      id-token: write
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo deploy
```

## `runtime-monitoring`

Jobs should start with a runtime monitor such as harden-runner.

**Non-compliant**

`tests/fixtures/rules/runtime-monitoring.before.yml`

```yaml
name: Monitoring
on: [push]
permissions: {}
jobs:
  build: # expect: runtime-monitoring
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
  hardened:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: step-security/harden-runner@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          egress-policy: audit
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
  reuse:
    uses: ./.github/workflows/reusable.yml
  # harden-runner needs sudo on the VM, so it cannot monitor a container job on a hosted runner:
  # https://github.com/step-security/harden-runner/blob/main/docs/limitations.md
  in-container:
    runs-on: ubuntu-24.04
    container: node:22@sha256:0000000000000000000000000000000000000000000000000000000000000000
    timeout-minutes: 10
    steps:
      - run: npm test
  slim:
    runs-on: ubuntu-slim
    timeout-minutes: 10
    steps:
      - run: echo slim
  self-hosted-container: # expect: runtime-monitoring
    runs-on: [self-hosted, linux]
    container: node:22@sha256:0000000000000000000000000000000000000000000000000000000000000000
    timeout-minutes: 10
    steps:
      - run: npm test
```

**Compliant**

`tests/fixtures/rules/runtime-monitoring.clean.yml`

```yaml
name: Monitoring
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: step-security/harden-runner@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          egress-policy: audit
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  reuse:
    uses: ./.github/workflows/reusable.yml
```

## `runner-versioning`

`runs-on` should name a fixed image version, not `-latest`.

**Non-compliant**

`tests/fixtures/rules/runner-versioning.before.yml`

```yaml
name: Runner labels
on: [push]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-latest # expect: runner-versioning
    timeout-minutes: 10
    steps:
      - run: echo a
  b:
    runs-on: [windows-latest] # expect: runner-versioning
    timeout-minutes: 10
    steps:
      - run: echo b
  c:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo c
  d:
    strategy:
      matrix:
        os: [ubuntu-24.04, macos-latest]
    runs-on: ${{ matrix.os }} # expect: runner-versioning
    timeout-minutes: 10
    steps:
      - run: echo d
```

**Compliant**

`tests/fixtures/rules/runner-versioning.clean.yml`

```yaml
name: Runner labels
on: [push]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo a
  b:
    runs-on: [windows-2025]
    timeout-minutes: 10
    steps:
      - run: echo b
```

## `missing-timeout`

Jobs should set `timeout-minutes`.

**Non-compliant**

`tests/fixtures/rules/missing-timeout.before.yml`

```yaml
name: Job timeouts
on: [push]
permissions: { contents: read }
jobs:
  build: # expect: missing-timeout
    runs-on: [self-hosted, linux]
    steps:
      - run: cargo build --release
      - run: cargo test
  verify:
    runs-on: ubuntu-22.04
    timeout-minutes: 20
    steps:
      - run: cargo fmt --check
  deploy: # expect: missing-timeout
    runs-on: ubuntu-22.04
    environment: staging
    steps:
      - run: echo 'Deploy fixture'
  reuse:
    uses: ./.github/workflows/reusable.yml
```

**Compliant**

`tests/fixtures/rules/missing-timeout.clean.yml`

```yaml
name: Job timeouts
on: [push]
permissions: { contents: read }
jobs:
  build:
    runs-on: [self-hosted, linux]
    timeout-minutes: 15
    steps:
      - run: cargo build --release
      - run: cargo test
  verify:
    runs-on: ubuntu-22.04
    timeout-minutes: 20
    steps:
      - run: cargo fmt --check
  deploy:
    runs-on: ubuntu-22.04
    environment: staging
    timeout-minutes: 10
    steps:
      - run: echo 'Deploy fixture'
  reuse:
    uses: ./.github/workflows/reusable.yml
```

## `error-suppression`

`continue-on-error: true` hides failures.

**Non-compliant**

`tests/fixtures/rules/error-suppression.before.yml`

```yaml
name: Errors
on: [push]
permissions: {}
jobs:
  a:
    continue-on-error: true # expect: error-suppression
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make lint
        continue-on-error: true # expect: error-suppression
      - run: make test
        continue-on-error: ${{ github.event_name == 'schedule' }}
      - run: make build
        continue-on-error: false
```

**Compliant**

`tests/fixtures/rules/error-suppression.clean.yml`

```yaml
name: Errors
on: [push]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make lint
      - run: make test
        continue-on-error: ${{ github.event_name == 'schedule' }}
      - run: make build
```

## `missing-concurrency`

CI workflows should cancel superseded runs with a concurrency group.

**Non-compliant**

`tests/fixtures/rules/missing-concurrency.before.yml`

```yaml
name: CI
on: [push, pull_request] # expect: missing-concurrency
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make test
```

**Compliant**

`tests/fixtures/rules/missing-concurrency.clean.yml`

```yaml
name: CI
on: [push, pull_request]
permissions: {}
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make test
```

## `broad-trigger`

push and pull_request triggers should filter branches, tags, or paths.

**Non-compliant**

`tests/fixtures/rules/broad-trigger.before.yml`

```yaml
name: Triggers
on:
  push: # expect: broad-trigger
  pull_request: # expect: broad-trigger
    types: [opened, synchronize]
  workflow_dispatch:
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make test
```

**Compliant**

`tests/fixtures/rules/broad-trigger.clean.yml`

```yaml
name: Triggers
on:
  push:
    branches: [main]
  pull_request:
    types: [opened, synchronize]
    paths: ['src/**']
  workflow_dispatch:
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make test
```

## `missing-fork-guard`

Scheduled jobs should skip forks.

**Non-compliant**

`tests/fixtures/rules/missing-fork-guard.before.yml`

```yaml
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
permissions: {}
jobs:
  nightly: # expect: missing-fork-guard
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  guarded:
    if: github.repository == 'example/project'
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  owner-guarded:
    if: ${{ github.repository_owner == 'example' }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  fork-flag-guarded:
    if: ${{ !github.event.repository.fork }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  full-name-guarded:
    if: github.event.repository.full_name == 'example/project'
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  publish:
    needs: guarded
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make publish
  announce:
    needs: [nightly, publish]
    if: success()
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make announce
  report: # expect: missing-fork-guard
    needs: guarded
    if: always()
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make report
  cleanup: # expect: missing-fork-guard
    needs: publish
    if: ${{ !cancelled() }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make cleanup
  mixed-case-guarded:
    if: GitHub.Repository == 'example/project'
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  reversed-guarded:
    if: ${{ 'example/project' == github.repository }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  fork-flag-false:
    if: github.event.repository.fork == false
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  schedule-or-success: # expect: missing-fork-guard
    needs: guarded
    if: success() || github.event_name == 'schedule'
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  fork-only: # expect: missing-fork-guard
    if: github.repository != 'example/project'
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  not-fork-flag: # expect: missing-fork-guard
    if: github.event.repository.fork
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  negated-guard: # expect: missing-fork-guard
    if: ${{ !(github.repository == 'example/project') }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  negated-fork-only:
    if: ${{ !(github['repository'] != 'example/project') }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
```

**Compliant**

`tests/fixtures/rules/missing-fork-guard.clean.yml`

```yaml
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
permissions: {}
jobs:
  nightly:
    if: github.repository == 'example/project'
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make nightly
  publish:
    needs: nightly
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make publish
  report:
    needs: publish
    if: ${{ always() && github.repository == 'example/project' }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make report
```

## `missing-matrix`

CI should test more than one OS or toolchain version.

**Non-compliant**

`tests/fixtures/rules/missing-matrix.before.yml`

```yaml
name: CI
on: [push]
permissions: {}
jobs: # expect: missing-matrix
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make test
```

**Compliant**

`tests/fixtures/rules/missing-matrix.clean.yml`

```yaml
name: CI
on: [push]
permissions: {}
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-24.04, windows-2025, macos-15]
    runs-on: ${{ matrix.os }}
    timeout-minutes: 10
    steps:
      - run: make test
```

## `cache-misuse`

Use the setup action's `cache` input instead of a separate actions/cache step.

**Non-compliant**

`tests/fixtures/rules/cache-misuse.before.yml`

```yaml
name: Cache
on: [push]
permissions: {}
jobs:
  node:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/setup-node@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: actions/cache@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: cache-misuse
        with:
          path: ~/.npm
          key: npm
  rust:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/cache@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          path: ~/.cargo
          key: cargo
  dotnet:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/setup-dotnet@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: actions/cache@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: cache-misuse
        with:
          path: ~/.nuget/packages
          key: nuget
```

**Compliant**

`tests/fixtures/rules/cache-misuse.clean.yml`

```yaml
name: Cache
on: [push]
permissions: {}
jobs:
  node:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/setup-node@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          cache: npm
  rust:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/cache@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          path: ~/.cargo
          key: cargo
  dotnet:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/setup-dotnet@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          cache: true
```

## `duplicated-steps`

A job should not repeat a step verbatim.

**Non-compliant**

`tests/fixtures/rules/duplicated-steps.before.yml`

```yaml
name: Duplicates
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          persist-credentials: false
      - run: make test
      - name: Test again
        run: make test # expect: duplicated-steps
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: duplicated-steps
        with:
          persist-credentials: false
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          persist-credentials: false
          path: other
  regenerate:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - id: before
        run: echo "sha=$(git rev-parse HEAD)" >> "$GITHUB_OUTPUT"
      - run: make generate
      - id: after
        run: echo "sha=$(git rev-parse HEAD)" >> "$GITHUB_OUTPUT"
      - if: steps.before.outputs.sha != steps.after.outputs.sha
        run: echo "generate moved HEAD"
      - name: Snapshot again
        run: echo "sha=$(git rev-parse HEAD)" >> "$GITHUB_OUTPUT" # expect: duplicated-steps
  bench:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - id: t1
        run: ./clock.sh
      - run: make work
      - id: t2
        run: ./clock.sh
      - run: echo "took $(( ${{ steps['t2'].outputs.ms }} - ${{ steps['t1'].outputs.ms }} ))"
  measure:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - id: m1
        run: ./measure.sh
      - id: m2
        run: ./measure.sh # expect: duplicated-steps
      - run: |
          # steps.m2 used to feed the summary.
          echo done
```

**Compliant**

`tests/fixtures/rules/duplicated-steps.clean.yml`

```yaml
name: Duplicates
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          persist-credentials: false
      - run: make test
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          persist-credentials: false
          path: other
  regenerate:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - id: before
        run: echo "sha=$(git rev-parse HEAD)" >> "$GITHUB_OUTPUT"
      - run: make generate
      - id: after
        run: echo "sha=$(git rev-parse HEAD)" >> "$GITHUB_OUTPUT"
      - if: steps.before.outputs.sha != steps.after.outputs.sha
        run: echo "generate moved HEAD"
```

## `complex-job`

Jobs, scripts, and conditions should stay small enough to review.

**Non-compliant**

`tests/fixtures/rules/complex-job.before.yml`

```yaml
name: Complexity
on: [push]
permissions: {}
jobs:
  long: # expect: complex-job
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo step 0
      - run: echo step 1
      - run: echo step 2
      - run: echo step 3
      - run: echo step 4
      - run: echo step 5
      - run: echo step 6
      - run: echo step 7
      - run: echo step 8
      - run: echo step 9
      - run: echo step 10
      - run: echo step 11
      - run: echo step 12
      - run: echo step 13
      - run: echo step 14
      - run: echo step 15
      - run: echo step 16
      - run: echo step 17
      - run: echo step 18
      - run: echo step 19
      - run: echo step 20
  script:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: | # expect: complex-job
          echo line 0
          echo line 1
          echo line 2
          echo line 3
          echo line 4
          echo line 5
          echo line 6
          echo line 7
          echo line 8
          echo line 9
          echo line 10
          echo line 11
          echo line 12
          echo line 13
          echo line 14
          echo line 15
          echo line 16
          echo line 17
          echo line 18
          echo line 19
          echo line 20
      - if: github.event_name == 'push' && github.ref == 'refs/heads/main' && github.actor != 'bot' || github.run_attempt > 1 # expect: complex-job
        run: echo conditional
      - if: github.event_name == 'push' && github.ref == 'refs/heads/main'
        run: echo simple
```

**Compliant**

`tests/fixtures/rules/complex-job.clean.yml`

```yaml
name: Complexity
on: [push]
permissions: {}
jobs:
  long:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: ./scripts/steps.sh
  script:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: ./scripts/lines.sh
      - if: github.event_name == 'push' && github.ref == 'refs/heads/main'
        run: echo simple
```

## `readability`

Workflows and run steps should have names.

**Non-compliant**

`tests/fixtures/rules/readability.before.yml`

```yaml
on: [push] # expect: readability
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
      - run: make test # expect: readability
      - name: Lint
        run: make lint
```

**Compliant**

`tests/fixtures/rules/readability.clean.yml`

```yaml
name: Tests
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - name: Test
        run: make test
```

## `deprecated-command`

Deprecated workflow commands and removed opt-out variables must not be used.

**Non-compliant**

`tests/fixtures/rules/deprecated-command.before.yml`

```yaml
name: Deprecated
on: [push]
permissions: {}
env:
  ACTIONS_ALLOW_UNSECURE_COMMANDS: true # expect: deprecated-command
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    env:
      ACTIONS_ALLOW_UNSECURE_COMMANDS: false
    steps:
      - run: echo "::set-output name=x::1" # expect: deprecated-command
      - run: echo "::save-state name=y::2" # expect: deprecated-command
      - run: | # expect: deprecated-command
          echo "::add-path::/opt/bin"
      - run: echo "x=1" >> "$GITHUB_OUTPUT"
      - run: echo "::add-mask::value"
      # A comment that names a command runs nothing.
      - run: |
          # migrated from ::set-output to GITHUB_OUTPUT
          echo "x=1" >> "$GITHUB_OUTPUT"
      - run: |
          echo "x=1" >> "$GITHUB_OUTPUT" # was ::set-output
      - run: | # expect: deprecated-command
          # still uses the old form
          echo "::set-env name=A::1"
      - run: | # expect: deprecated-command
          printf '%s\n' '::save-state name=a # b::1'
      - run: ./legacy-tool
        env:
          ACTIONS_ALLOW_UNSECURE_COMMANDS: 'true' # expect: deprecated-command
      # The runner reads the variable with .NET bool.TryParse: `True` in any case, nothing else.
      - run: ./legacy-tool --upper
        env:
          ACTIONS_ALLOW_UNSECURE_COMMANDS: 'TRUE' # expect: deprecated-command
      - run: ./legacy-tool --expression
        env:
          ACTIONS_ALLOW_UNSECURE_COMMANDS: ${{ 'true' }} # expect: deprecated-command
      - run: ./legacy-tool --spelled-out
        env:
          ACTIONS_ALLOW_UNSECURE_COMMANDS: FALSE
      - run: ./legacy-tool --not-a-boolean
        env:
          ACTIONS_ALLOW_UNSECURE_COMMANDS: 'yes'
```

**Compliant**

`tests/fixtures/rules/deprecated-command.clean.yml`

```yaml
name: Deprecated
on: [push]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    env:
      ACTIONS_ALLOW_UNSECURE_COMMANDS: false
    steps:
      - run: echo "x=1" >> "$GITHUB_OUTPUT"
      - run: echo "y=2" >> "$GITHUB_STATE"
      - run: echo "/opt/bin" >> "$GITHUB_PATH"
      - run: echo "::add-mask::value"
      - run: ./legacy-tool
```

## `removed-feature`

Retired runner images, shut-down action versions, and removed action inputs must not be used.

**Non-compliant**

`tests/fixtures/rules/removed-feature.before.yml`

```yaml
name: Retired
on: [push]
permissions: {}
jobs:
  linux:
    runs-on: ubuntu-20.04 # expect: removed-feature
    timeout-minutes: 10
    steps:
      - uses: actions/upload-artifact@v3 # expect: removed-feature
        with: { name: dist, path: dist }
      - uses: Actions/Upload-Artifact@v3 # expect: removed-feature
        with: { name: logs, path: logs }
      - uses: actions/download-artifact@c850b930e6ba138125429b7e5c93fc707a7f8427 # v4.1.4
      - uses: actions/download-artifact@9bc31d5ccc31df68ecc42ccf4149144866c47d8a # v3.0.2 # expect: removed-feature
      - uses: actions/cache@v2 # expect: removed-feature
        with: { path: ~/.cargo, key: cargo }
      - uses: actions/cache@v3.3.1 # expect: removed-feature
        with: { path: ~/.npm, key: npm }
      - uses: actions/cache@v4.1.2 # expect: removed-feature
        with: { path: ~/.m2, key: m2 }
      - uses: actions/cache@v3
        with: { path: ~/.gradle, key: gradle }
      - uses: actions/cache@v4.2.0
        with: { path: ~/.ivy2, key: ivy }
  matrix:
    strategy:
      matrix:
        os: [ubuntu-24.04, windows-2019]
    runs-on: ${{ matrix.os }} # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo matrix
  included:
    strategy:
      matrix:
        os: [ubuntu-24.04]
        include:
          - os: ubuntu-20.04
            legacy: true
    runs-on: ${{ matrix.os }} # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo include
  self-hosted:
    runs-on: [self-hosted, ubuntu-20.04]
    timeout-minutes: 10
    steps:
      - run: echo self-hosted
  windows:
    runs-on: [windows-2019] # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo windows
  mac:
    runs-on: macos-13 # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo mac
  group:
    runs-on: { group: builders, labels: [macos-13] }
    timeout-minutes: 10
    steps:
      - run: echo group
  old:
    runs-on: ubuntu-18.04 # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo old
  xenial:
    runs-on: ubuntu-16.04 # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo xenial
  server-2016:
    runs-on: windows-2016 # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo server-2016
  catalina:
    runs-on: macos-10.15 # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo catalina
  big-sur:
    runs-on: macos-11 # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo big-sur
  monterey:
    runs-on: macos-12 # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo monterey
  nested:
    strategy:
      matrix:
        config: [{ os: ubuntu-24.04 }, { os: ubuntu-20.04 }]
    runs-on: ${{ matrix.config.os }} # expect: removed-feature
    timeout-minutes: 10
    steps:
      - run: echo nested
```

**Compliant**

`tests/fixtures/rules/removed-feature.clean.yml`

```yaml
name: Retired
on: [push]
permissions: {}
jobs:
  linux:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/upload-artifact@v4
        with: { name: dist, path: dist }
      - uses: actions/cache@v4.2.0
        with: { path: ~/.cargo, key: cargo }
  windows:
    runs-on: windows-2025
    timeout-minutes: 10
    steps:
      - uses: actions/setup-java@v6
        with:
          distribution: temurin
          java-version: 17
  mac:
    runs-on: macos-15
    timeout-minutes: 10
    steps:
      - run: echo mac
```

## `schema-validity`

Workflows must use only the keys GitHub's workflow syntax defines.

**Non-compliant**

`tests/fixtures/rules/schema-validity.before.yml`

```yaml
name: Schema
on: [push]
permissions: {}
permisions: read-all # expect: schema-validity
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    timeout: 10 # expect: schema-validity
    steps:
      - name: Both # expect: schema-validity
        uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
        run: echo both
      - name: Neither # expect: schema-validity
      - run: echo ok
        working-dir: src # expect: schema-validity
      - parallel:
          - run: echo one
          - run: echo two
      - run: sleep 5
        id: sleeper
        background: true
      - wait: [sleeper]
  orphan: # expect: schema-validity
    timeout-minutes: 10
    steps:
      - run: echo orphan
```

**Compliant**

`tests/fixtures/rules/schema-validity.clean.yml`

```yaml
name: Schema
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Checkout
        uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - run: echo ok
        working-directory: src
      - parallel:
          - run: echo one
          - run: echo two
      - run: sleep 5
        id: sleeper
        background: true
      - wait: [sleeper]
```

## `embedded-script-lint`

Environment file paths must be quoted in shell scripts.

**Non-compliant**

`tests/fixtures/rules/embedded-script-lint.before.yml`

```yaml
name: Shell
on: [push]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo "a=1" >> $GITHUB_ENV # expect: embedded-script-lint
      - run: echo "b=2" >> "$GITHUB_OUTPUT"
      - run: | # expect: embedded-script-lint
          echo "## Summary" >> ${GITHUB_STEP_SUMMARY}
      - shell: pwsh
        run: echo "c=3" >> $env:GITHUB_ENV
      - shell: /usr/bin/bash -e {0}
        run: echo "d=4" >> $GITHUB_ENV # expect: embedded-script-lint
      - shell: /bin/sh -eu {0}
        run: echo "e=5" >> $GITHUB_OUTPUT # expect: embedded-script-lint
      - shell: /usr/bin/pwsh -command ". '{0}'"
        run: echo "f=6" >> $env:GITHUB_ENV
      - shell: /usr/bin/env bash -e {0}
        run: echo "g=7" >> $GITHUB_ENV # expect: embedded-script-lint
      - shell: /usr/bin/env -S pwsh -File {0}
        run: echo "h=8" >> $env:GITHUB_ENV
  # `run` defaults to pwsh on Windows runners, where `$GITHUB_ENV` word splitting does not apply:
  # https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idstepsshell
  windows:
    runs-on: windows-2025
    timeout-minutes: 10
    steps:
      - run: echo "a=1" >> $GITHUB_ENV
      - shell: bash
        run: echo "a=1" >> $GITHUB_ENV # expect: embedded-script-lint
  self-hosted-windows:
    runs-on: [self-hosted, Windows, X64]
    timeout-minutes: 10
    steps:
      - run: echo "a=1" >> $GITHUB_ENV
  both:
    strategy:
      matrix:
        os: [ubuntu-24.04, windows-2025]
    runs-on: ${{ matrix.os }}
    timeout-minutes: 10
    steps:
      - run: echo "a=1" >> $GITHUB_ENV # expect: embedded-script-lint
```

**Compliant**

`tests/fixtures/rules/embedded-script-lint.clean.yml`

```yaml
name: Shell
on: [push]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo "a=1" >> "$GITHUB_ENV"
      - run: echo "b=2" >> "$GITHUB_OUTPUT"
      - run: |
          echo "## Summary" >> "${GITHUB_STEP_SUMMARY}"
      - shell: pwsh
        run: echo "c=3" >> $env:GITHUB_ENV
      - shell: /usr/bin/bash -e {0}
        run: echo "d=4" >> "$GITHUB_ENV"
      - shell: /bin/sh -eu {0}
        run: echo "e=5" >> "$GITHUB_OUTPUT"
```

## `insecure-protocol`

URLs must use an encrypted scheme.

**Non-compliant**

`tests/fixtures/rules/insecure-protocol.before.yml`

```yaml
name: URLs
on: [push]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    services:
      httpstatus:
        image: ghcr.io/example/httpstatus@sha256:0000000000000000000000000000000000000000000000000000000000000000
    env:
      ASPNETCORE_URLS: http://+:80
    steps:
      - run: curl -fsSL http://example.com/tool.tar.gz -o tool.tgz # expect: insecure-protocol
      - run: curl http://localhost:8080/health
      - run: curl http://[::1]:8080/health
      - run: curl http://[2001:db8::1]/tool.tar.gz # expect: insecure-protocol
      # Service containers and other single-label hosts never leave the runner's network.
      - run: curl http://httpstatus/200
      - run: curl http://${{ env.MIRROR }}/tool.tar.gz # expect: insecure-protocol
      - uses: example/deploy@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          registry: http://registry.example.com # expect: insecure-protocol
      - run: git clone git://example.com/repo.git # expect: insecure-protocol
      - run: curl https://example.com
```

**Compliant**

`tests/fixtures/rules/insecure-protocol.clean.yml`

```yaml
name: URLs
on: [push]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    services:
      httpstatus:
        image: ghcr.io/example/httpstatus@sha256:0000000000000000000000000000000000000000000000000000000000000000
    env:
      ASPNETCORE_URLS: http://+:80
    steps:
      - run: curl -fsSL https://example.com/tool.tar.gz -o tool.tgz
      - run: curl http://localhost:8080/health
      - run: curl http://[::1]:8080/health
      - run: curl https://[2001:db8::1]/tool.tar.gz
      - run: curl http://httpstatus/200
      - run: curl https://${{ env.MIRROR }}/tool.tar.gz
      - uses: example/deploy@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          registry: https://registry.example.com
      - run: git clone https://example.com/repo.git
```

## `unpinned-action`

Actions, reusable workflows, and container images must be pinned to a full commit SHA or image digest.

**Non-compliant**

`tests/fixtures/rules/unpinned-action.before.yml`

```yaml
# Synthetic references for syntax-only checks; do not run this workflow.
name: Reference pinning
on: [push]
permissions: { contents: read }
jobs:
  test:
    runs-on: ubuntu-22.04
    timeout-minutes: 15
    container:
      image: ghcr.io/example/build:stable # expect: unpinned-action
    services:
      database:
        image: postgres:16 # expect: unpinned-action
    steps:
      - uses: example/setup@v2 # expect: unpinned-action
      - uses: example/build@abc1234 # expect: unpinned-action
      - uses: docker://alpine:3.20 # expect: unpinned-action
      - uses: example/pinned@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: ./.github/actions/local
      - uses: docker://alpine@sha256:1111111111111111111111111111111111111111111111111111111111111111
  reuse:
    uses: example/shared/.github/workflows/build.yml@main # expect: unpinned-action
  local-reuse:
    uses: ./.github/workflows/build.yml
  pinned-reuse:
    uses: example/shared/.github/workflows/build.yml@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
```

**Compliant**

`tests/fixtures/rules/unpinned-action.clean.yml`

```yaml
# Synthetic references for syntax-only checks; do not run this workflow.
name: Reference pinning
on: [push]
permissions: { contents: read }
jobs:
  test:
    runs-on: ubuntu-22.04
    timeout-minutes: 15
    container:
      image: ghcr.io/example/build@sha256:2222222222222222222222222222222222222222222222222222222222222222
    services:
      database:
        image: postgres@sha256:3333333333333333333333333333333333333333333333333333333333333333
    steps:
      - uses: example/setup@cccccccccccccccccccccccccccccccccccccccc
      - uses: example/build@dddddddddddddddddddddddddddddddddddddddd
      - uses: docker://alpine@sha256:4444444444444444444444444444444444444444444444444444444444444444
      - uses: example/pinned@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: ./.github/actions/local
      - uses: docker://alpine@sha256:1111111111111111111111111111111111111111111111111111111111111111
  reuse:
    uses: example/shared/.github/workflows/build.yml@eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
  local-reuse:
    uses: ./.github/workflows/build.yml
  pinned-reuse:
    uses: example/shared/.github/workflows/build.yml@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
```

## `forbidden-action`

`uses` must match the configured allowlist or denylist.

**Non-compliant**

`tests/fixtures/rules/forbidden-action.before.yml`

```yaml
name: Configured
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
      - uses: tj-actions/changed-files@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: forbidden-action
      - uses: tj-actions/verify-changed-files/sub@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa # expect: forbidden-action
      - uses: docker://alpine:3.20
```

**Compliant**

`tests/fixtures/rules/forbidden-action.clean.yml`

```yaml
name: Configured
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: docker://alpine:3.20
```

## `self-hosted-runner`

Self-hosted runners must not run jobs from pull requests or issues.

**Non-compliant**

`tests/fixtures/rules/self-hosted-runner.before.yml`

```yaml
name: Self-hosted
on: [pull_request]
permissions: {}
jobs:
  a:
    runs-on: self-hosted # expect: self-hosted-runner
    timeout-minutes: 10
    steps:
      - run: echo a
  b:
    runs-on: [self-hosted, linux] # expect: self-hosted-runner
    timeout-minutes: 10
    steps:
      - run: echo b
  c:
    runs-on: # expect: self-hosted-runner
      group: private
      labels: [self-hosted, gpu]
    timeout-minutes: 10
    steps:
      - run: echo c
  d:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo d
  # Self-hosted runners get OS and architecture labels by default; hosted runners never use them:
  # https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/use-in-a-workflow
  e:
    runs-on: [Linux, X64] # expect: self-hosted-runner
    timeout-minutes: 10
    steps:
      - run: echo e
  f:
    runs-on: ARM64 # expect: self-hosted-runner
    timeout-minutes: 10
    steps:
      - run: echo f
  g:
    runs-on: [windows-2025, ubuntu-24.04-arm]
    timeout-minutes: 10
    steps:
      - run: echo g
  h:
    strategy:
      matrix:
        runner: [ubuntu-24.04, [self-hosted, linux]]
    runs-on: ${{ matrix.runner }} # expect: self-hosted-runner
    timeout-minutes: 10
    steps:
      - run: echo h
  i:
    strategy:
      matrix:
        runner: [ubuntu-24.04]
        include:
          - runner: [self-hosted, gpu]
    runs-on: ${{ matrix.runner }} # expect: self-hosted-runner
    timeout-minutes: 10
    steps:
      - run: echo i
  j:
    strategy:
      matrix:
        runner: [[ubuntu-24.04], [windows-2025]]
    runs-on: ${{ matrix.runner }}
    timeout-minutes: 10
    steps:
      - run: echo j
  k:
    strategy:
      matrix:
        config: [{ os: ubuntu-24.04 }, { os: self-hosted }]
    runs-on: ${{ matrix.config.os }} # expect: self-hosted-runner
    timeout-minutes: 10
    steps:
      - run: echo k
  l:
    strategy:
      matrix:
        config: [{ os: ubuntu-24.04 }]
        include:
          - config: { os: windows-2025 }
    runs-on: ${{ matrix.config.os }}
    timeout-minutes: 10
    steps:
      - run: echo l
```

**Compliant**

`tests/fixtures/rules/self-hosted-runner.clean.yml`

```yaml
name: Self-hosted
on: [pull_request]
permissions: {}
jobs:
  a:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo a
  h:
    strategy:
      matrix:
        runner: [[ubuntu-24.04], [windows-2025]]
    runs-on: ${{ matrix.runner }}
    timeout-minutes: 10
    steps:
      - run: echo h
```

## `known-vulnerable-action`

Referenced action versions must not have published advisories.

**Non-compliant**

`tests/fixtures/online/known-vulnerable-action.before.yml`

```yaml
name: Online
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: tj-actions/changed-files@v35 # expect: known-vulnerable-action
      - uses: tj-actions/changed-files@1111111111111111111111111111111111111111 # v35.0.0 # expect: known-vulnerable-action
      - uses: actions/checkout@v4
        with: { persist-credentials: false }
      - uses: example/unversioned@main
      - uses: example/moving@v4
      - uses: example/moving@v4.0.0 # expect: known-vulnerable-action
```

**Compliant**

`tests/fixtures/online/known-vulnerable-action.clean.yml`

```yaml
name: Online
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: tj-actions/changed-files@v46.0.5
      - uses: actions/checkout@v4
      - uses: example/moving@v4
```

## `outdated-action`

Referenced action versions should not lag behind the latest release.

**Non-compliant**

`tests/fixtures/online/outdated-action.before.yml`

```yaml
name: Online
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v3 # expect: outdated-action
        with: { persist-credentials: false }
      - uses: actions/checkout@v4
        with: { persist-credentials: false }
      - uses: actions/setup-node@4444444444444444444444444444444444444444 # v4.0.0 # expect: outdated-action
      - uses: actions/setup-node@4444444444444444444444444444444444444444 # v4.1.0
```

**Compliant**

`tests/fixtures/online/outdated-action.clean.yml`

```yaml
name: Online
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@4444444444444444444444444444444444444444 # v4.1.0
```

## `version-comment-mismatch`

The version comment next to a pinned SHA must name that SHA's tag.

**Non-compliant**

`tests/fixtures/online/version-comment-mismatch.before.yml`

```yaml
name: Online
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v4.2.2
        with: { persist-credentials: false }
      - uses: actions/checkout@2222222222222222222222222222222222222222 # v4.2.2 # expect: version-comment-mismatch
        with: { persist-credentials: false }
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v9.9.9 # expect: version-comment-mismatch
        with: { persist-credentials: false }
```

**Compliant**

`tests/fixtures/online/version-comment-mismatch.clean.yml`

```yaml
name: Online
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v4.2.2
```

## `impostor-commit`

A pinned SHA must belong to the named repository, not to a fork.

**Non-compliant**

`tests/fixtures/online/impostor-commit.before.yml`

```yaml
name: Online
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v4.2.2
        with: { persist-credentials: false }
      - uses: actions/checkout@2222222222222222222222222222222222222222 # expect: impostor-commit
        with: { persist-credentials: false }
      - uses: actions/checkout@3333333333333333333333333333333333333333
        with: { persist-credentials: false }
      - uses: example/many-tags@abababababababababababababababababababab
      - uses: example/many-tags@cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd # expect: impostor-commit
```

**Compliant**

`tests/fixtures/online/impostor-commit.clean.yml`

```yaml
name: Online
on:
  push:
    branches: [main]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v4.2.2
      - uses: actions/checkout@3333333333333333333333333333333333333333
      - uses: example/many-tags@abababababababababababababababababababab
```

## `archived-action`

Actions from archived repositories receive no security fixes.

**Non-compliant**

`tests/fixtures/rules/archived-action.before.yml`

```yaml
name: Archived actions offline
on: [push]
permissions: {}
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/create-release@0cb9c9b65d5d1901c1f53e5e66eaf4afd303e70e # v1.1.4 # expect: archived-action
      - uses: Actions-RS/Toolchain@v1 # expect: archived-action
      - uses: actions-rs/cargo@v1 # expect: archived-action
      # Unknown offline: only online mode can tell whether other repositories are archived.
      - uses: example/abandoned-release@v1
      - uses: actions/checkout@v5
        with: { persist-credentials: false }
```

**Compliant**

`tests/fixtures/rules/archived-action.clean.yml`

```yaml
name: Archived actions offline
on: [push]
permissions: {}
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: softprops/action-gh-release@v2
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --locked
      - uses: actions/checkout@v5
        with: { persist-credentials: false }
```

## `missing-release-cooldown`

Dependabot updates should wait for a minimum release age.

**Non-compliant**

`tests/fixtures/repos/missing-release-cooldown/.github/dependabot.yml`

```yaml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
```

**Compliant**

`tests/fixtures/repos/hardened/.github/dependabot.yml`

```yaml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
```

## `missing-security-tooling`

Repositories should run dependency updates, CodeQL, dependency review, and OpenSSF Scorecard.

**Non-compliant**

`tests/fixtures/repos/missing-security-tooling/.github/workflows/ci.yml`

```yaml
name: CI
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Test
        run: make test
```

**Compliant**

`tests/fixtures/repos/hardened/.github/dependabot.yml`

```yaml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
```

`tests/fixtures/repos/hardened/.github/workflows/security.yml`

```yaml
name: Security
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
permissions: {}
jobs:
  codeql:
    permissions:
      security-events: write
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    steps:
      - uses: github/codeql-action/init@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - uses: github/codeql-action/analyze@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  review:
    if: github.event_name == 'pull_request'
    permissions:
      contents: read
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/dependency-review-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  scorecard:
    permissions:
      security-events: write
      id-token: write
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: ossf/scorecard-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

## `retired-runtime`

JavaScript actions must not declare a retired Node.js runtime.

**Non-compliant**

`tests/fixtures/rules/retired-runtime.before.yml`

```yaml
name: Retired runtimes
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4 # expect: retired-runtime
        with: { persist-credentials: false }
      - uses: Actions/Setup-Node@v3 # expect: retired-runtime
      - uses: actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065 # v5.6.0 # expect: retired-runtime
      - uses: actions/cache/restore@v4.2.0 # expect: retired-runtime
        with: { path: ~/.npm, key: npm }
      - uses: actions/github-script@v7 # expect: retired-runtime
        with: { script: "core.info('x')" }
      - uses: actions/upload-artifact@v4 # expect: retired-runtime
        with: { name: dist, path: dist }
      - uses: actions/create-release@v1 # expect: retired-runtime
      # v1 of checkout ran as a runner plugin, not on Node.js.
      - uses: actions/checkout@v1
        with: { persist-credentials: false }
      # Latest v3 artifact releases were rebuilt on node24; removed-feature reports the shutdown.
      - uses: actions/upload-artifact@v3
        with: { name: logs, path: logs }
      - uses: actions/checkout@main
        with: { persist-credentials: false }
      - uses: actions/upload-pages-artifact@v3
      - uses: ./.github/actions/local
      - uses: docker://alpine:3.20
```

**Compliant**

`tests/fixtures/rules/retired-runtime.clean.yml`

```yaml
name: Current runtimes
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v5
        with: { persist-credentials: false }
      - uses: actions/setup-node@v5
      - uses: actions/setup-python@v6
      - uses: actions/cache/restore@v5
        with: { path: ~/.npm, key: npm }
      - uses: actions/github-script@v8
        with: { script: "core.info('x')" }
      - uses: actions/upload-artifact@v6
        with: { name: dist, path: dist }
```

## `repo-org-settings`

Repository Actions settings must not grant write tokens or pull request approval by default.

This rule reads the repository's Actions settings through the GitHub API, so no YAML shows it.
Its test responses are in `tests/fixtures/online/api.json`.
