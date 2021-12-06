#!/bin/bash

ABSPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"
FILE="ayame-rs"
PIDF="ayame.pid"
CONFIG="config.toml"

function main() {
	if [ "$1" == "start" ]; then
		start
	elif [ "$1" == "stop" ]; then
		stop
	elif [ "$1" == "update" ]; then
		stop
		update
		start
	else
		printf "No argument provided\n"
		exit -1
	fi
}

function stop() {
	kill -SIGINT "$(cat $PIDF)"
	if [[ ! -f "$PIDF" ]]; then
		rm $PIDF
	fi
}

function start() {
	if [[ -f "$PIDF" ]]; then
		printf "Ayame already running\n"
		exit -1
	fi
	if [[ ! -f "$FILE" ]]; then
		cargo build --release
		cp ./target/release/$FILE .
	fi
	./$FILE > "$(date -I).log" & echo $! > ./$PIDF
}

function update() {
	git fetch --all && git pull
	cargo build --release
	cp ./target/release/$FILE $FILE
}



funtion check_config() {
	if [[ ! -f "$CONFIG" ]]; then
		printf "Your bot token: "
		read TOKEN
		echo "token = \"$TOKEN\"" > $CONFIG
		printf "\nYour application id (usually your bot user id): "
		read APP_ID
		echo "application_id = $APP_ID" >> $CONFIG
		printf "\nYour prefix: "
		read PREFIX
		echo "prefix = \"$PREFIX\"" >> $CONFIG
	fi

}

cd $ABSPATH
check_config
main $1
