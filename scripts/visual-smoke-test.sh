#!/usr/bin/env bash

set -Eeuo pipefail

readonly APP_ID="io.github.Gor3pig.Pigoune.Devel"
readonly OUTPUT_DIR="${PIGOUNE_VISUAL_OUTPUT_DIR:-/tmp/pigoune-visual-test}"

xvfb_pid=""
app_session_pid=""
app_instance=""
window_id=""
test_display=""
existing_instances=()

cleanup() {
    set +e

    if [[ -n "$app_instance" ]]; then
        flatpak kill "$app_instance" 2>/dev/null

        for _ in {1..50}; do
            if ! instance_is_running "$app_instance"; then
                break
            fi
            sleep 0.1
        done
    fi

    if [[ -n "$app_session_pid" ]]; then
        kill -TERM -- "-$app_session_pid" 2>/dev/null

        for _ in {1..20}; do
            if ! kill -0 "$app_session_pid" 2>/dev/null; then
                break
            fi
            sleep 0.1
        done

        kill -KILL -- "-$app_session_pid" 2>/dev/null
        wait "$app_session_pid" 2>/dev/null
    fi

    if [[ -n "$xvfb_pid" ]]; then
        kill -TERM "$xvfb_pid" 2>/dev/null
        wait "$xvfb_pid" 2>/dev/null
    fi
}

trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

require_command() {
    local command_name="$1"

    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'Erreur : la commande requise « %s » est introuvable.\n' "$command_name" >&2
        exit 1
    fi
}

choose_display() {
    local display_number

    for display_number in {90..99}; do
        if [[ -e "/tmp/.X11-unix/X${display_number}" || -e "/tmp/.X${display_number}-lock" ]]; then
            continue
        fi

        if DISPLAY=":${display_number}" xdotool getdisplaygeometry >/dev/null 2>&1; then
            continue
        fi

        printf ':%s\n' "$display_number"
        return 0
    done

    printf 'Erreur : aucun DISPLAY libre trouvé entre :90 et :99.\n' >&2
    return 1
}

wait_for_display() {
    for _ in {1..50}; do
        if DISPLAY="$test_display" xdotool getdisplaygeometry >/dev/null 2>&1; then
            return 0
        fi

        if ! kill -0 "$xvfb_pid" 2>/dev/null; then
            printf "Erreur : Xvfb s'est arrêté avant d'être prêt. Consultez %s/xvfb.log.\n" "$OUTPUT_DIR" >&2
            return 1
        fi

        sleep 0.1
    done

    printf 'Erreur : Xvfb ne répond pas sur %s. Consultez %s/xvfb.log.\n' "$test_display" "$OUTPUT_DIR" >&2
    return 1
}

wait_for_window() {
    for _ in {1..100}; do
        window_id="$(DISPLAY="$test_display" xdotool search --onlyvisible --name '^Pigoune$' 2>/dev/null | head -n 1 || true)"
        if [[ -n "$window_id" ]]; then
            return 0
        fi

        if ! kill -0 "$app_session_pid" 2>/dev/null; then
            printf "Erreur : Pigoune s'est arrêté avant de créer sa fenêtre. Consultez %s/pigoune.log.\n" "$OUTPUT_DIR" >&2
            return 1
        fi

        sleep 0.1
    done

    printf 'Erreur : aucune fenêtre Pigoune détectée sur %s. Consultez %s/pigoune.log.\n' "$test_display" "$OUTPUT_DIR" >&2
    return 1
}

list_app_instances() {
    local instance application

    while read -r instance application; do
        if [[ "$application" == "$APP_ID" ]]; then
            printf '%s\n' "$instance"
        fi
    done < <(flatpak ps --columns=instance,application 2>/dev/null)
}

instance_existed_before_test() {
    local candidate="$1"
    local existing

    for existing in "${existing_instances[@]}"; do
        if [[ "$candidate" == "$existing" ]]; then
            return 0
        fi
    done

    return 1
}

instance_is_running() {
    local candidate="$1"
    local running

    while read -r running; do
        if [[ "$candidate" == "$running" ]]; then
            return 0
        fi
    done < <(list_app_instances)

    return 1
}

wait_for_app_instance() {
    local candidate
    local found
    local found_count

    for _ in {1..50}; do
        found=""
        found_count=0

        while read -r candidate; do
            if ! instance_existed_before_test "$candidate"; then
                found="$candidate"
                ((found_count += 1))
            fi
        done < <(list_app_instances)

        if ((found_count == 1)); then
            app_instance="$found"
            return 0
        fi

        if ((found_count > 1)); then
            printf 'Erreur : plusieurs nouvelles instances Pigoune ont été détectées ; nettoyage ciblé impossible.\n' >&2
            return 1
        fi

        sleep 0.1
    done

    printf "Erreur : impossible d'identifier l'instance Flatpak Pigoune du test.\n" >&2
    return 1
}

capture_window() {
    local width="$1"
    local height="$2"
    local filename="$3"

    DISPLAY="$test_display" xdotool windowsize --sync "$window_id" "$width" "$height"
    sleep 1
    DISPLAY="$test_display" import -window "$window_id" "$OUTPUT_DIR/$filename"

    if [[ ! -s "$OUTPUT_DIR/$filename" ]]; then
        printf 'Erreur : la capture %s est vide.\n' "$OUTPUT_DIR/$filename" >&2
        return 1
    fi
}

for dependency in Xvfb xdotool dbus-run-session import flatpak setsid; do
    require_command "$dependency"
done

if ! flatpak info "$APP_ID" >/dev/null 2>&1; then
    printf "Erreur : le Flatpak %s n'est pas installé.\n" "$APP_ID" >&2
    exit 1
fi

mapfile -t existing_instances < <(list_app_instances)

mkdir -p "$OUTPUT_DIR"
rm -f \
    "$OUTPUT_DIR/wide.png" \
    "$OUTPUT_DIR/medium.png" \
    "$OUTPUT_DIR/narrow.png" \
    "$OUTPUT_DIR/wide-restored.png" \
    "$OUTPUT_DIR/xvfb.log" \
    "$OUTPUT_DIR/pigoune.log"

test_display="$(choose_display)"

Xvfb "$test_display" -screen 0 1600x1000x24 -nolisten tcp -ac \
    >"$OUTPUT_DIR/xvfb.log" 2>&1 &
xvfb_pid="$!"
wait_for_display

setsid env \
    DISPLAY="$test_display" \
    GDK_BACKEND=x11 \
    WAYLAND_DISPLAY=wayland-pigoune-headless-none \
    dbus-run-session -- \
    flatpak run --no-documents-portal --env=GDK_BACKEND=x11 "$APP_ID" \
    >"$OUTPUT_DIR/pigoune.log" 2>&1 &
app_session_pid="$!"

wait_for_window
wait_for_app_instance

window_title="$(DISPLAY="$test_display" xdotool getwindowname "$window_id")"
printf 'Fenêtre détectée : %s (ID %s, instance %s) sur %s.\n' \
    "$window_title" "$window_id" "$app_instance" "$test_display"

capture_window 1280 800 wide.png
capture_window 1000 800 medium.png
capture_window 700 800 narrow.png
capture_window 1280 800 wide-restored.png

printf 'Captures produites dans %s :\n' "$OUTPUT_DIR"
printf '  %s\n' wide.png medium.png narrow.png wide-restored.png
