var socket;
var conn_div;
var m_conn_div;

var num_msgs;
var next_del;

// Limit for messages displayed in the output window (will trim from the beginning past limit) //
// 	- Should be kept reasonably small (< 100-ish) to prevent 
//	  the browser from clogging on all of the data 
var msg_limit = 50;

var mode;
var stopped;
var logging = false;
var output_mouse_over = false;
var smooth_scroll = false;

var h_pct, h_raw, m_pct, m_raw, e_pct, e_raw, w_pct, w_raw, nl_pct, nl_raw;

var m_nw, m_n, m_ne, m_e, m_se, m_s, m_sw, m_w, m_up, m_down, m_in, m_out;

var map;

var WEBSOCKET_KEY_COUNT = 0;
if (WebSocket) {
	if (WebSocket.prototype) {
		for (WEBSOCKET_KEY in WebSocket.prototype) {
			WEBSOCKET_KEY_COUNT = WEBSOCKET_KEY_COUNT + 1;
		}
	}
}

// Change this as necessary //
var wshost = "";
if (WEBSOCKET_KEY_COUNT < 10) {
	wshost = WNG_LEGACY_PREFIX + "://" + WNG_LEGACY_HOST + ":" + WNG_LEGACY_CONNECTION_PORT + "/";
} else {
	wshost = WNG_NORMAL_PREFIX + "://" + window.location.host + window.location.pathname.substring(0, lastIndexOf(window.location.pathname, "/")) + "/ws";
}

function lastIndexOf(str, needle) {
    var lastIndex = 0;
    var tmpIndex = 0;
	var string = str;
    while (true) {
		tmpIndex = string.indexOf(needle);
		if (tmpIndex == -1) break;
		lastIndex += tmpIndex + 1;
		string = string.substring(tmpIndex + 1);
    }
    return lastIndex - 1;
}

var do_json_parse;
if (JSON && JSON.parse) {
	do_json_parse = JSON.parse;
} else {
	do_json_parse = json_parse;
}

var mudhost;
var mudport;

var scrollback, prevent_autoscroll=false;

var objDiv;

$(document).ready(function(){		    
	conn_div = document.getElementById('connection_ws');
	m_conn_div = document.getElementById('connection_mud');	
	
	m_nw = document.getElementById("m_nw");
	m_n = document.getElementById("m_n");
	m_ne = document.getElementById("m_ne");
	m_e = document.getElementById("m_e");
	m_se = document.getElementById("m_se");
	m_s = document.getElementById("m_s");
	m_sw = document.getElementById("m_sw");
	m_w = document.getElementById("m_w");
	m_up = document.getElementById("m_u");
	m_down = document.getElementById("m_d");
	m_in = document.getElementById("m_i");
	m_out = document.getElementById("m_o");
	
	h_pct = document.getElementById("h_pct");
	h_raw = document.getElementById("h_raw");
	m_pct = document.getElementById("m_pct");
	m_raw = document.getElementById("m_raw");
	e_pct = document.getElementById("e_pct");
	e_raw = document.getElementById("e_raw");
	w_pct = document.getElementById("w_pct");
	w_raw = document.getElementById("w_raw");
	nl_pct = document.getElementById("nl_pct");
	nl_raw = document.getElementById("nl_raw");

	objDiv = document.getElementById("output");
	
	num_msgs = 0;	
	next_del = 0;	
	
	scrollback = function() {
		//var objDiv = window.top.document.getElementById("output");
		if (objDiv.scrollTop < objDiv.scrollHeight && output_mouse_over)
		{	
			$("#output").css("border-color", "red");			
			$("#autoscroll").show();
			prevent_autoscroll = true;
		} else {
			prevent_autoscroll = false;			
		}
	}
	
	$("#output").dblclick(scrollback);
	
	$("#output").hover(
		function() {
			output_mouse_over = true;			
		},
		function() {
			output_mouse_over = false;
			prevent_autoscroll = false;
			$("#output").css("border-color", "#464646");
					
			//var objDiv = window.top.document.getElementById("output");					
			objDiv.scrollTop = objDiv.scrollHeight;
			$("#autoscroll").hide();
		}		
	);
	
	 // Set URL of your WebSocketMain.swf here:
    WebSocket.__swfLocation = "WebSocketMain.swf";
    // Set this to dump debug message from Flash to console.log:
    WebSocket.__debug = true;	    
	
	mode = "websocket";
	
	socket = new WebSocket(wshost);	
	
	socket.onopen = function() {
    	set_connected_phudbase();
    }
	
	socket.onmessage = function(evt) {				
			handle_read(evt.data);
	}
    
    socket.onclose = function(evt) {
    	set_disconnected();
    }      
    
    socket.onerror = function() {
       ow_Write("<p>WebSocket error:</p>");
    }

	if (!( filterXSS && (filterXSS("<span>") == "<span>") )) {
		print("Warning: filterXSS not supported on this browser.", "tnc_white");
	}
});	

function sendDirect(data) 
{
   	if (data != "")
	{	
   		if (mode == "websocket")	   			
   			socket.send(data);
   		else
   			sendTextToFlash(data);
		return true;
	} else {
		return false;
	}	
}

function send() 
{
	s = document.getElementById("user_input").value;

	print(s, "tnc_grey");
	
	if (mode == "websocket")
		socket.send(s);
	else
		sendTextToFlash(s);
	
	document.getElementById("user_input").value = "";
	return true;	
}

function postLogin() 
{
	document.getElementById("user_input").value = "";
	$("#data_form").fadeIn(500, function() {document.getElementById("user_input").focus()});	
	$("#login_area").remove();	
}

function set_connected_phudbase()
{
	conn_div.style.background = "green";
	conn_div.innerHTML = "<p style='font-size: 1.5em; font-weight: bold; color: #fff;'>CONNECTED</p>";
}

function set_connected_mud()
{
	m_conn_div.style.background = "green";
	m_conn_div.innerHTML = "<p style='font-size: 1.5em; font-weight: bold; color: #fff;'>CONNECTED</p>";	
}

function set_disconnected_mud()
{
	m_conn_div.style.background = "red";
	m_conn_div.innerHTML = "<p style='font-size: 1.25em; font-weight: bold; color: #fff;'>DISCONNECTED</p>";
}

function set_disconnected()
{
	conn_div.style.background = "red";
    conn_div.innerHTML = "<p style='font-size: 1.25em; font-weight: bold; color: #fff;'>DISCONNECTED</p>";
    m_conn_div.style.background = "red";
	m_conn_div.innerHTML = "<p style='font-size: 1.25em; font-weight: bold; color: #fff;'>DISCONNECTED</p>";
}

function print(s) {
	    
    var color = "tnc_light_grey";
    if (typeof(arguments[1]) != undefined)
        color = arguments[1];
    
    ow_Write("<br><span class='" + color + "'>&rarr; " + s + "</span><br>");
}

function handle_read(s)
{
	//console.log(s);
	//data = eval("(" + s + ")");
	data = do_json_parse(s);

	// Check for ATCP messages ( //
	// Unimplemented.
	//handle_ATCP(data);
	
	// Output a standard message //
	if (data.message) ow_Write(data.message);
	
	// Write a WebMud server status message //
	if (data.server_status) ss_Write(data.server_status);
	
	// Set the connection status for the MUD
	if (data.conn_status) 
	{
		if (data.conn_status == "connected")		
			set_connected_mud();
		else if (data.conn_status == "disconnected")
			set_disconnected_mud();		
	}
	
	// Set the connection status for the PHudBase-WebMud server (sent by the Flash client) //
	if (data.fconn_status)
	{
		if (data.fconn_status == "connected")		
			set_connected_phudbase();
		 else if (data.fconn_status == "disconnected")
			set_disconnected();		
	}
}

function ow_Write(text)
{	
	//var objDiv = window.top.document.getElementById("output");

	if (filterXSS) {
		text = filterXSS(text, {
			"whiteList": {
				"span": ["class"],
				"br": []
			}
		});
	}

	text = '<span id="msg' + num_msgs + '">' + text + '</span>'; 

	if (objDiv.insertAdjacentHTML) {
		objDiv.insertAdjacentHTML("beforeend", text);
	} else {
		objDiv.innerHTML += text;
	}
	
	trim_ow();
	
	num_msgs++;
	
	if (prevent_autoscroll == true) return;
	
	objDiv.scrollTop = objDiv.scrollHeight;
}

function trim_ow()
{
	var elem;	
	
	if (num_msgs >= msg_limit)
	{		
		elem = "#msg" + next_del;
			
		$(elem).remove();
		
		next_del++;
	}
}
