var evtSource = new EventSource("/events");
evtSource.onmessage = (msg) => { location.reload(); }
