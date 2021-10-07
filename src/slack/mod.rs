use serde::{Deserialize as De, Serialize as Ser};

/// Payload sent by slack on slash commands.
///
/// [https://api.slack.com/interactivity/slash-commands#responding_to_commands]
///
/// An example payload (converted from form data to JSON):
/// ```json
/// {
///   "api_app_id": "A02HEGWV9AM",
///   "channel_id": "C71L81P1V",
///   "channel_name": "general",
///   "command": "/deploy",
///   "is_enterprise_install": "false",
///   "response_url": "https://hooks.slack.com/commands/T70FCJL9Z/2583280222628/NR6ppC3PF3wJ9WBc6ws2BsVP",
///   "team_domain": "orion-dev-playground",
///   "team_id": "T70FCJL9Z",
///   "text": "imercata staging",
///   "token": "TKOK5oe76LmuuFJ8Uh3kNZdh",
///   "trigger_id": "2593691513185.238522632339.c6af97bbed3f8498ff35a728c68dc0a8",
///   "user_id": "U71L81N15",
///   "user_name": "cakekindel"
/// }
/// ```
#[derive(Ser, De, Debug)]
pub struct SlashCommand {
  /// The command that was typed in to trigger this request. This value can be useful if you want to use a single Request URL to service multiple Slash Commands, as it lets you tell them apart.
  pub command: String,
  /// These IDs provide context about where the user was in Slack when they triggered your app's command (eg. which workspace, Enterprise Grid, or channel). You may need these IDs for your command response.
  ///
  /// The various accompanying *_name values provide you with the plain text names for these IDs, but as always you should only rely on the IDs as the names might change arbitrarily.
  ///
  /// We'll include enterprise_id and enterprise_name parameters on command invocations when the executing workspace is part of an Enterprise Grid.
  pub channel_id: String,
  /// See docs for `channel_id`
  pub team_id: String,
  /// A temporary webhook URL that you can use to generate messages responses.
  pub response_url: String,
  /// Slack workspace URL
  pub team_domain: String,
  /// This is the part of the Slash Command after the command itself, and it can contain absolutely anything that the user might decide to type. It is common to use this text parameter to provide extra context for the command.
  ///
  /// You can prompt users to adhere to a particular format by showing them in the Usage Hint field when creating a command.
  pub text: String,
  /// The ID of the user who triggered the command.
  pub user_id: String,
}
