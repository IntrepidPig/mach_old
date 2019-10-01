var playerId = null;
var gameId = null;
var gameName = null;
var taskmanReady = true;
var statusLine = null;
var statusLineText = "";

function setStatusLine(text) {
	statusLineText = text;
	if (statusLine !== null) {
		statusLine.innerHTML = text;
	}
}

var logText = "";

function setGameName(name) {
	gameName = name;
	var gameNameElement = document.getElementById("game-name-header");
	gameNameElement.innerHTML = name;
}

function scoreIncreaseButtonClicked() {
	if (playerId === null) {
		return;
	}
	
	sendRequest("POST", "/game_call", {"IncreaseScore":{"player": playerId}}, function(req, err) {
		
	});
}

function hostGameButtonClicked() {
	sendRequest("POST", "/game_call", {"HostGame":{"player":playerId}}, function(req, err) {
		var obj = JSON.parse(req.responseText);
		setGameName(obj["HostGameResponse"]["game_name"]);
		gameId = obj["HostGameResponse"]["id"];
	});
}

function joinGameButtonClicked() {
	var targetGameName = document.getElementById("game-name-input").value;
	sendRequest("POST", "/game_call", {"JoinGame":{"player":playerId,game_name: targetGameName}}, function(req, err) {
		var obj = JSON.parse(req.responseText);
		if (obj["JoinGameResponse"]["success"]) {
			gameId = obj["JoinGameResponse"]["game_id"];
			setGameName(targetGameName);
		} else {
			setStatusLine("Failed to join game");
		}
	});
}

function init() {
	statusLine = document.getElementById("status-line");
	
	sendRequest("POST", "/game_call", {"Register":{"name":"Bobdor"}}, function(req, err) {
		var obj = JSON.parse(req.responseText);
		playerId = obj["RegisterResponse"]["id"];
	});
	
	var taskman = setInterval(function() {
		if (gameId !== null && playerId !== null) {
			sendRequest("POST", "/game_call", {"StateCheck": {"player": playerId, "game": gameId}}, function(req, err) {
				var obj = JSON.parse(req.responseText);
				logText += JSON.stringify(obj) + "\n\n";
				var gameLogsElement = document.getElementById("game-logs");
				gameLogsElement.innerHTML = logText;
				if (obj["StateCheckResponse"]["InGame"]) {
					var p1ScoreElement = document.getElementById("p1score");
					var p2ScoreElement = document.getElementById("p2score");
					p1ScoreElement.innerHTML = obj["StateCheckResponse"]["InGame"]["player1_score"].toString();
					p2ScoreElement.innerHTML = obj["StateCheckResponse"]["InGame"]["player2_score"].toString();
					var currentPlayerElement = null;
					if (obj["StateCheckResponse"]["InGame"]["player"] === 1) {
						currentPlayerElement = p1ScoreElement;
					} else {
						currentPlayerElement = p2ScoreElement;
					}
					currentPlayerElement.setAttribute("style", "color:blue;");
				}
			})
		}
	}, 1000);
}

function sendRequest(method, path, obj, callback) {
	var req = new XMLHttpRequest();
	req.onreadystatechange = function() {
		if (req.readyState === XMLHttpRequest.DONE) {
			if (req.status === 200) {
				callback(req, false)
			} else {
				callback(req, true)
			}
		}
	}
	req.open(method, path, true);
	req.send(JSON.stringify(obj))
}