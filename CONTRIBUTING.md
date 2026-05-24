# Contributing to llm-manager

Thank you for your interest in contributing to `llm-manager`! We welcome contributions of all kinds, including bug reports, feature requests, documentation improvements, and code changes.

## Reporting Bugs

If you find a bug, please open an issue on GitHub. Include:
- A clear description of the problem.
- Steps to reproduce the issue.
- Your operating system and hardware configuration (especially GPU details).
- Any relevant log output (check the Log panel with `F6`).

## Suggesting Features

Feature requests are welcome! Please open an issue to discuss your ideas before starting implementation.

## Development Setup

To set up your development environment:

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/aginies/llmtui.git
    cd llmtui
    ```

2.  **Use the build script:**
    A convenience script `./build.sh` is provided for common development tasks:
    - `./build.sh check`: Run `cargo check`.
    - `./build.sh format`: Format code using `cargo fmt`.
    - `./build.sh clippy`: Run `cargo clippy`.
    - `./build.sh test`: Run tests.
    - `./build.sh build`: Build a debug binary.
    - `./build.sh run`: Build and run the TUI.

## Code Standards

- **Formatting:** Always run `./build.sh format` before committing.
- **Linting:** Ensure `./build.sh clippy` passes without warnings.
- **Testing:** Add tests for new features or bug fixes whenever possible and ensure `./build.sh test` passes.
- **Commit Messages:** Use clear, descriptive commit messages.

## AI Assistance

- Do not post output from Large Language Models or similar generative AI as comments on GitHub or our discourse server, as such comments tend to be formulaic and low content.
- If you use generative AI tools as an aid in developing code or documentation changes, ensure that you fully understand the proposed changes and can explain why they are the correct approach.
- Make sure you have added value based on your personal competency to your contributions. Just taking some input, feeding it to an AI and posting the result is not of value to the project. To preserve precious core developer capacity, we reserve the right to rigorously reject seemingly AI generated low-value contributions.
- It is also strictly forbidden to post AI generated content to issues or PRs via automated tooling such as bots or agents. We may ban such users and/or report them to GitHub.
- You are responsible for every line of code submitted. This project is human driven, so we don't do changes because an AI assist says that the code could be improved in any area.

## Pull Request Process

1.  Create a new branch for your changes.
2.  Follow the development setup and code standards above.
3.  Submit a pull request with a detailed description of your changes.
4.  Participate in the code review process.

## License

By contributing to this project, you agree that your contributions will be licensed under the project's GPLv3 license.
