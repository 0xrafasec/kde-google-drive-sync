# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_gdrivesync_global_optspecs
	string join \n json q/quiet v/verbose h/help V/version
end

function __fish_gdrivesync_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_gdrivesync_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_gdrivesync_using_subcommand
	set -l cmd (__fish_gdrivesync_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -s V -l version -d 'Print version'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "status" -d 'Per-account status, quota, last sync (requires running daemon)'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "accounts"
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "sync"
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "folders"
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "errors" -d 'Recent sync errors from daemon DB'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "quota" -d 'Drive storage quota per account'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "daemon"
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "completions" -d 'Print shell completions (bash, zsh, fish)'
complete -c gdrivesync -n "__fish_gdrivesync_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand status" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand status" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand status" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand status" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and not __fish_seen_subcommand_from list add remove help" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and not __fish_seen_subcommand_from list add remove help" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and not __fish_seen_subcommand_from list add remove help" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and not __fish_seen_subcommand_from list add remove help" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and not __fish_seen_subcommand_from list add remove help" -f -a "list" -d 'List configured accounts'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and not __fish_seen_subcommand_from list add remove help" -f -a "add" -d 'Run OAuth via daemon (browser opens); blocks until done'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and not __fish_seen_subcommand_from list add remove help" -f -a "remove" -d 'Remove account and revoke token. Asks for confirmation unless --yes'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and not __fish_seen_subcommand_from list add remove help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from list" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from list" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from list" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from add" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from add" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from add" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from add" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from remove" -s y -l yes
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from remove" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from remove" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from remove" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from remove" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from help" -f -a "list" -d 'List configured accounts'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from help" -f -a "add" -d 'Run OAuth via daemon (browser opens); blocks until done'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from help" -f -a "remove" -d 'Remove account and revoke token. Asks for confirmation unless --yes'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand accounts; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and not __fish_seen_subcommand_from pause resume now help" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and not __fish_seen_subcommand_from pause resume now help" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and not __fish_seen_subcommand_from pause resume now help" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and not __fish_seen_subcommand_from pause resume now help" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and not __fish_seen_subcommand_from pause resume now help" -f -a "pause" -d 'Pause all sync folders'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and not __fish_seen_subcommand_from pause resume now help" -f -a "resume" -d 'Resume sync'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and not __fish_seen_subcommand_from pause resume now help" -f -a "now" -d 'Queue immediate sync (optional path = restrict to folder under path)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and not __fish_seen_subcommand_from pause resume now help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from pause" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from pause" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from pause" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from pause" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from resume" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from resume" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from resume" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from resume" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from now" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from now" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from now" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from now" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from help" -f -a "pause" -d 'Pause all sync folders'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from help" -f -a "resume" -d 'Resume sync'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from help" -f -a "now" -d 'Queue immediate sync (optional path = restrict to folder under path)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand sync; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and not __fish_seen_subcommand_from list add remove help" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and not __fish_seen_subcommand_from list add remove help" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and not __fish_seen_subcommand_from list add remove help" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and not __fish_seen_subcommand_from list add remove help" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and not __fish_seen_subcommand_from list add remove help" -f -a "list" -d 'List sync folder mappings'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and not __fish_seen_subcommand_from list add remove help" -f -a "add" -d 'Add mapping (uses first account; see daemon docs)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and not __fish_seen_subcommand_from list add remove help" -f -a "remove"
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and not __fish_seen_subcommand_from list add remove help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from list" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from list" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from list" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from add" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from add" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from add" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from add" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from remove" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from remove" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from remove" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from remove" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from help" -f -a "list" -d 'List sync folder mappings'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from help" -f -a "add" -d 'Add mapping (uses first account; see daemon docs)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from help" -f -a "remove"
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand folders; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand errors" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand errors" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand errors" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand errors" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand quota" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand quota" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand quota" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand quota" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and not __fish_seen_subcommand_from start stop status help" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and not __fish_seen_subcommand_from start stop status help" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and not __fish_seen_subcommand_from start stop status help" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and not __fish_seen_subcommand_from start stop status help" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and not __fish_seen_subcommand_from start stop status help" -f -a "start" -d 'Start daemon (systemd --user if available, else spawn gds-daemon)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and not __fish_seen_subcommand_from start stop status help" -f -a "stop" -d 'Stop daemon (SIGTERM to PID file or systemctl)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and not __fish_seen_subcommand_from start stop status help" -f -a "status" -d 'Show whether daemon is on D-Bus and PID file'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and not __fish_seen_subcommand_from start stop status help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from start" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from start" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from start" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from start" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from stop" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from stop" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from stop" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from stop" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from status" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from status" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from status" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from help" -f -a "start" -d 'Start daemon (systemd --user if available, else spawn gds-daemon)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from help" -f -a "stop" -d 'Stop daemon (SIGTERM to PID file or systemctl)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from help" -f -a "status" -d 'Show whether daemon is on D-Bus and PID file'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand daemon; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand completions" -l json -d 'Machine-readable JSON on stdout (stable field names)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand completions" -s q -l quiet -d 'Suppress non-error stdout'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand completions" -s v -l verbose -d 'Extra diagnostics (implies info-level logging if RUST_LOG unset)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand completions" -s h -l help -d 'Print help'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "status" -d 'Per-account status, quota, last sync (requires running daemon)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "accounts"
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "sync"
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "folders"
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "errors" -d 'Recent sync errors from daemon DB'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "quota" -d 'Drive storage quota per account'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "daemon"
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "completions" -d 'Print shell completions (bash, zsh, fish)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and not __fish_seen_subcommand_from status accounts sync folders errors quota daemon completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from accounts" -f -a "list" -d 'List configured accounts'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from accounts" -f -a "add" -d 'Run OAuth via daemon (browser opens); blocks until done'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from accounts" -f -a "remove" -d 'Remove account and revoke token. Asks for confirmation unless --yes'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from sync" -f -a "pause" -d 'Pause all sync folders'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from sync" -f -a "resume" -d 'Resume sync'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from sync" -f -a "now" -d 'Queue immediate sync (optional path = restrict to folder under path)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from folders" -f -a "list" -d 'List sync folder mappings'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from folders" -f -a "add" -d 'Add mapping (uses first account; see daemon docs)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from folders" -f -a "remove"
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from daemon" -f -a "start" -d 'Start daemon (systemd --user if available, else spawn gds-daemon)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from daemon" -f -a "stop" -d 'Stop daemon (SIGTERM to PID file or systemctl)'
complete -c gdrivesync -n "__fish_gdrivesync_using_subcommand help; and __fish_seen_subcommand_from daemon" -f -a "status" -d 'Show whether daemon is on D-Bus and PID file'
