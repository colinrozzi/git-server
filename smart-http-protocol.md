Git Smart HTTP Protocol Specification (Client/Server)

Overview

Git’s “smart” HTTP protocol is the modern, efficient way to transfer data over HTTP/HTTPS, requiring a Git-aware server process. Unlike the “dumb” HTTP protocol (which serves static files), the smart protocol uses a custom request/response exchange on top of HTTP. This allows clients to clone, fetch, and push with full Git semantics over standard HTTP(S) ports ￼ ￼. The smart protocol is stateless from the HTTP server’s perspective – each request is independent, and all session state is managed by the client ￼. Authentication (if any) is handled by HTTP (e.g. Basic auth), not by the Git protocol itself ￼.

Dumb vs. Smart HTTP: With dumb HTTP, a client just downloads files (like objects and refs) directly via HTTP GET requests. With smart HTTP, the client and server speak a Git-specific packfile protocol over HTTP. Smart clients may try the smart protocol first and fall back to dumb if the server doesn’t support smart HTTP ￼.

Packet Line Format (pkt-line)

All Git smart protocol messages are exchanged in the pkt-line format. Each message (or “packet”) is prefixed by a 4-byte hexadecimal length (including the 4 bytes of the length itself). For example, a packet with payload "hi\n" (3 bytes) would have length 0007 (7 = 4+3 bytes) and appear on the wire as 0007hi\n. A special length of 0000 is a flush-pkt, indicating the end of a message stream or section ￼ ￼. (Another special length 0001 is a delimiter pkt used only in protocol v2, not needed for v1.) The maximum pkt-line length is 65520 bytes (0xFFF0 in hex) ￼.

Examples: 001e# service=git-upload-pack\n is a pkt-line of length 0x001E (30 bytes) containing the text “# service=git-upload-pack” plus newline ￼. A flush packet is represented as 0000 with no following data. The client and server must generate and parse these exactly; any deviation in lengths or format will cause communication failure ￼.

Reference Discovery: GET info/refs

Before any fetch or push, a smart HTTP client discovers the refs on the server. This is done by requesting the special info/refs endpoint with a query parameter specifying the service. The client sends:

GET $GIT_URL/info/refs?service=<servicename> HTTP/1.1

Where <servicename> is either git-upload-pack (for fetch/clone) or git-receive-pack (for push) ￼ ￼. For example, a fetch client would request:

GET /repo.git/info/refs?service=git-upload-pack

Server Response: If the repository exists and the service is enabled, the server responds 200 OK with content type application/x-<servicename>-advertisement ￼ (e.g. application/x-git-upload-pack-advertisement for fetch). The body of the response is the ref advertisement, encoded as a series of pkt-lines:
	•	The first pkt-line is a welcome banner: # service=<servicename> (prefixed by length). For example: 001e# service=git-upload-pack\n ￼.
	•	This is followed by a flush-pkt (0000) to mark the end of the banner section ￼.
	•	Next is the ref listing: each reference is sent as a pkt-line of the form: <object-id> <refname> (with an LF). The refs are usually sorted by name (C locale order) ￼. If the repository has a HEAD, the HEAD ref is listed first ￼.
	•	Capabilities: On the first ref line, after the ref name, there is a NUL byte (\0) followed by a space-separated list of server capabilities ￼ ￼. These capabilities advertise optional protocol features (multi_ack, thin-pack, etc.) that the server supports – the client can choose from these in its request. (If the repository has no refs at all, a dummy ref capabilities^{} with a zero-id is sent to carry the capability list ￼.)
	•	Finally, a flush-pkt (0000) is sent to terminate the ref list ￼.

Example: A smart upload-pack advertisement might look like:

Content-Type: application/x-git-upload-pack-advertisement

001e# service=git-upload-pack\n
0000
0088<HEAD_OBJ_ID> HEAD\0multi_ack thin-pack side-band-64k ofs-delta shallow agent=git/2.34.1\n
003f<branch_obj_id> refs/heads/master\n
003f<tag_obj_id> refs/tags/v1.0\n
0000

Here the first ref line lists HEAD (with its current object ID), followed by a NUL and capabilities multi_ack … ofs-delta etc. ￼ ￼. Subsequent lines list other refs (e.g. master, tags) without any capabilities. The response is terminated by a flush (0000). Clients must verify this format: the status is 200 (or 304 if cached) and the first bytes match “[0-9a-f]{4}#” (a pkt-line with “# service=…”). If not, clients should fall back to the dumb protocol or error out ￼.

(If the service is not enabled, the server should return 403 Forbidden ￼. If the repository is not found, a 404 is returned instead ￼.)

Fetch/Clone via git-upload-pack (Smart Fetch Service)

After reference discovery, a client can fetch objects (clone or pull) using the git-upload-pack service. This occurs via one or more HTTP POST requests to the git-upload-pack endpoint. The interaction can be summarized in steps:
	1.	Initial Request: The client sends a POST to "$GIT_URL/git-upload-pack" with header Content-Type: application/x-git-upload-pack-request ￼. The body of this POST is a series of pkt-lines known as the upload-pack request. This request tells the server which objects the client wants and what it already has:
	•	“want” lines: The client must send at least one want command identifying a commit (or other ref) it wants to fetch. Each want line is want <object-id> (40 hexadecimal characters for SHA-1) plus LF, in a pkt-line. All the want lines together form the “want list.” If the client wants multiple references, it sends multiple want lines. The first want line may include a NUL \0 followed by the list of capabilities the client is requesting (a subset of those the server advertised) ￼ ￼. For example, the first want might be: 0032want <sha1> multi_ack thin-pack side-band-64k ofs-delta\n. Subsequent wants (if any) are sent without capabilities (e.g. 0032want <sha1>\n) ￼.
	•	(Optional) shallow/depth lines: If doing a shallow clone, the client may send deepen <N> or deepen-since <timestamp> etc., as additional pkt-lines, to limit history depth (this is only if the server advertised shallow or related capabilities) ￼ ￼.
	•	After sending all desired want lines (and any deepen/shallow lines), the client ends the want list with a flush-pkt (0000) ￼. This flush indicates “end of wants; ready for server’s response.”
	2.	Server Acknowledgment (if depth/shallow): If a shallow clone was requested, the server responds immediately (in the HTTP response to this POST) with its own shallow info: a series of shallow <obj-id> and unshallow <obj-id> lines, ending with a flush, to tell the client which commits will be treated as shallow ￼ ￼. Otherwise, if no depth/shallow negotiation is needed, the server proceeds to the next step.
	3.	Client “have” negotiation: After the initial want-list (and processing any shallow response), the client will typically make another POST to continue negotiation if it has some common history with the server. In this next phase, the client informs the server of Git objects it already has, to allow the server to minimize the pack. The client sends a series of have lines: each have <object-id> identifies an object the client possesses (usually recent commits from the client’s refs) ￼ ￼. The client may send up to 256 have lines in one batch. After a batch of have lines, the client can end that batch with a flush-pkt (0000) to ask the server for acknowledgments ￼ ￼. The server will respond (HTTP 200 with content type application/x-git-upload-pack-result) with ACK messages for any have-object the server also has, or NAK if none are common yet ￼ ￼. For example, in multi_ack mode, the server might reply with lines like ACK <obj-id> continue for common commits and a final NAK if it still needs more info ￼ ￼. These ACK/NAK responses come as pkt-lines in the HTTP response body.
	4.	Iteration: The client and server may repeat step 3 (with additional POSTs) if needed. The client sends more have lines (in another POST) if the server indicated it still doesn’t have a common base (the server’s ACK ... continue means “I got that have, but continue sending more”). This back-and-forth continues until the client and server find enough common commits or the client has listed all its commits ￼. (Modern Git uses multi_ack or multi_ack_detailed to make this negotiation efficient.)
	5.	Completion (“done”): Once the client is satisfied that the server has enough information (or if the client has no more haves to give), the client sends a final POST to conclude the negotiation. This final request includes a done marker instead of a flush. For example, the last packet from client might be 0009done\n (which is a pkt-line containing “done”) ￼. Important: The done marker should not be preceded by an extra flush (no 0000 before done) – it directly follows any last have line ￼ ￼. The done tells the server to halt negotiation and prepare to send the pack data.
	6.	Packfile Response: Upon receiving done, the server responds with the packfile. The HTTP response to the final POST will have Content-Type: application/x-git-upload-pack-result and begins streaming the packfile data. Before the raw pack data, the server may send one last ACK (for the last common id) or a NAK line, depending on the ack mode ￼ ￼. After that, the packfile is sent. If the client requested side-band/side-band-64k in its capabilities, the packfile is sent multiplexed on sideband channel 1 (with progress messages on channel 2) ￼ ￼. In side-band mode, the pack data is split into pkt-lines each containing up to 65519 bytes of data plus a 1-byte channel code ￼ ￼. For example, a sideband packet might look like PACK<binary data> prefixed by length and channel. The client will demultiplex this: data channel 1 is the actual packfile bytes, channel 2 is informational/progress text (which clients typically show on stderr), and channel 3 (if used) indicates an error message ￼. If the client did not request side-band(-64k), the server just sends a raw packfile (starting with the four-byte "PACK" header) directly after the NAK/ACK ￼ ￼. The packfile format itself is the standard Git packfile (see pack-format documentation) containing the objects needed for the requested refs.
	7.	Client receives pack: The client will receive the packfile (applying any delta “thinning” if thin-pack was used) and finalize the fetch. At this point, the fetch or clone is complete – the client can index the received pack and update its refs.

Notes: The entire fetch process may involve multiple HTTP requests due to the stateless nature of HTTP. Each POST (with have lines or done) yields an HTTP response with ACK/NAK or final pack data ￼ ￼. Modern Git servers can advertise the capability no-done (along with multi_ack_detailed) to eliminate an extra round trip: with no-done, the server can send the pack immediately after sending an “ACK … ready” to the client, without waiting for a separate done request ￼. But a minimal implementation can operate without no-done by using the explicit done as described.

The server should include Cache-Control: no-cache on responses to ensure clients do not cache these results ￼ ￼. Clients also should not reuse any previous response; each fetch POST is fresh ￼.

Push via git-receive-pack (Smart Push Service)

Pushing to a remote uses the git-receive-pack service. The client will first discover refs as usual, then send a POST request to update refs and transfer objects. The push sequence is:
	1.	Reference Discovery: The client does GET $GIT_URL/info/refs?service=git-receive-pack similar to fetch. The server responds with Content-Type: application/x-git-receive-pack-advertisement and a ref advertisement in pkt-line format ￼ ￼. This advertisement lists the current refs and their values on the server, with capabilities applicable to receive-pack. For receive-pack, the typical advertised capabilities include things like report-status, delete-refs, ofs-delta, side-band-64k, quiet, atomic, and possibly push-options or agent ￼. (The set of capabilities for push is more limited – for example, you won’t see multi_ack here, since that’s for fetch ￼ ￼.) As with upload-pack, the first ref line contains the capability list after a NUL. Example first line: 003f<obj-id> refs/heads/master\0 report-status delete-refs side-band-64k atomic ofs-delta ... ￼.
	2.	Update Request (POST): The client then issues a POST $GIT_URL/git-receive-pack with Content-Type: application/x-git-receive-pack-request ￼. This request body has two parts:
	•	Command list: a series of pkt-line commands indicating which refs to update. Each command is of the form: <old-obj-id> <new-obj-id> <refname> (all in one line). For example, to update refs/heads/main from old value X to new value Y, the line is: X Y refs/heads/main. To create a new ref, the old id is all zeros; to delete a ref, the new id is all zeros ￼ ￼. If there are multiple refs being updated in one push, each is a separate command line. The first command line may carry client capabilities after a NUL \0. For instance, the first line might be:

<old-id> <new-id> <refname>\0 report-status side-band-64k

to request that the server provide a status report and allow side-band progress ￼. Subsequent command lines (if any) are just old new ref with no capabilities. After the last command, the client sends a flush-pkt (0000) to terminate the command list ￼ ￼.

	•	Packfile Data: Following the flush, the client sends a binary Git packfile containing all objects necessary to fulfill the reference updates. This packfile is typically prepared by the client’s git pack-objects. It contains new commits, trees, blobs, etc., that the server will need to have the objects for the new refs. The packfile starts with the bytes "PACK" and is not pkt-line encoded (it’s sent raw after the flush). If no new objects are needed (e.g. the client is deleting a branch or updating a ref to an existing object on the server), the client must still send an empty packfile header to complete the push (an empty pack with just header and trailer) ￼. Note: If the push was only deleting refs (no creates/updates), the client actually does not send a packfile at all ￼, since no object transfer is needed.

	3.	Server Processing: The server reads the commands and the packfile. It will unpack the packfile (validating it). Then for each ref command:
	•	Ensure the current server ref value matches the provided old-id (to detect races where the ref changed since advertisement).
	•	If the new-id is not zero, update the ref to the new value (if it is a fast-forward or forced update as per server policy); if new-id is zero, delete the ref (if allowed by server and if delete-refs was advertised).
	•	Run any hooks (pre-receive, update, post-receive) to validate or perform additional logic. The server will only actually update refs if hooks permit and all conditions are satisfied.
	4.	Result Report: If the client requested report-status (which it should for a proper push), the server will send back a report of what happened. The HTTP response will have Content-Type: application/x-git-receive-pack-result. The body is a series of pkt-lines (often using side-band channel 1 if that was negotiated):
	•	First, an unpack status line: either unpack ok or unpack <error message> ￼ ￼. This tells whether the packfile was unpacked successfully. If this says “error”, it means the push failed entirely (e.g. packfile was corrupt or hook rejected all changes).
	•	Then, for each ref command that was attempted, a command status line: ok <refname> if that ref was updated successfully, or ng <refname> <reason> if it failed ￼ ￼. “ng” (no good) reason might be “non-fast-forward” or a hook error, etc. The server can reject some refs and accept others in the same push, if atomic transactions are not in use ￼. If the atomic capability was requested, then the server will either accept all updates or none – in that case a failure for one ref means no refs were changed.
	•	Each of these status lines is a pkt-line. After listing all ref results, the server sends a flush-pkt (0000) to end the report ￼.
If the client also requested side-band-64k for push, the server may send this report over sideband channel 2 (for progress) or channel 3 (for errors) with multiplexing, although in practice many git-receive-pack implementations send the report plain or on channel 1. For example, if side-band was enabled, progress messages (like hook output or other info) could be sent on channel 2. The final “unpack ok”/“ok refs/… ” lines might be on channel 1 or just as plain text if side-band wasn’t used ￼ ￼. (Clients typically handle both possibilities.)
	5.	Push Completion: The client, after receiving the report-status, knows the outcome. If it sees unpack ok and all expected “ok ” lines, the push succeeded. If any “ng” lines or an unpack error are present, the push (or that ref) failed. At this point, the HTTP push request is done. The client will close the connection (or reuse it for another request if supported). The server has updated its repository as needed.

Note on Capabilities (Push): The push advertisement’s capability list informs the client what it may request. Common capabilities for receive-pack:
	•	report-status: Allows push status reporting (as above) ￼.
	•	report-status-v2: An enhanced status report (for newer clients/servers) – if used, the server will send a slightly different format, but basic implementers can stick to v1.
	•	delete-refs: Allows deletion of refs (old-id = 0000…0 to delete) ￼.
	•	ofs-delta: Server can accept packfiles with OFS deltas (almost always yes).
	•	side-band-64k: Allows use of side-band for progress/status.
	•	quiet: If client sends quiet, server will suppress human-readable progress output (useful for scripts) ￼.
	•	atomic: Allows atomic multi-ref updates ￼.
	•	push-options: Allows client to send additional push options (after the command list) which server passes to hooks ￼ ￼.
	•	agent: (Optional) The server may send its agent string, and client can respond with its agent.

If the client wants any of these, it should include them (space-separated) after the first ref update command (after a NUL). For example, a client pushing might include report-status ofs-delta side-band-64k if those were advertised by the server. A minimal but correct client should at least request report-status, otherwise it won’t know if the push succeeded. In practice, Git clients also request side-band-64k and ofs-delta on push for efficiency.

Additional Notes and Implementation Tips
	•	HTTP and URI details: The base repository URL ($GIT_URL) should end with .git (for bare repos) or be configured accordingly. The server must accept requests with the extra path components (info/refs, git-upload-pack, etc.) appended to this URL ￼ ￼. Ensure the server strips any trailing “/” in $GIT_URL to avoid // in paths ￼. For pushes, HTTP clients typically use HTTP authentication (Basic or Digest over HTTPS) – the server can rely on HTTP auth (e.g. web server or proxy) to grant/deny access ￼.
	•	Content-Length/Chunking: Clients and servers should handle HTTP/1.1 chunked transfer encoding, since these requests and responses often use it ￼. The examples above often show Transfer-Encoding: chunked for responses. When implementing, you can stream the pkt-lines and packfile without knowing total length by using chunked encoding.
	•	Error handling: If a server encounters an error (e.g. repository not found, or a forbidden service), it should return an HTTP error (404, 403, etc.) rather than a 200 with a malformed payload ￼ ￼. During the pkt-line exchanges, an error can also be signaled by sending an ERR <message> pkt-line to the client (this would typically be in place of an expected ACK/NAK or pack data) ￼. Clients will abort on receiving an ERR line.
	•	Packfile implementation: Implementing packfile reading/writing is one of the hardest parts. You will need to be able to create packfiles (for serving fetches) and parse/apply packfiles (for receiving pushes). The packfile format is documented (pack-format and pack-protocol docs) – it includes a header "PACK", a version, number of objects, and then a series of compressed objects (possibly deltas) followed by a trailer with a SHA-1 checksum. If writing this from scratch is too complex, you might consider using a library or existing implementation for packfile I/O. For Rust, low-level Git libraries like gitoxide have components for packfile encoding/decoding, although not all parts may compile to WASM. Ensure that any library used does not rely on OS features unavailable in WASM (e.g. threads or fork). Alternatively, the JGit project (Java) or libgit2 (C) might serve as references for the pack algorithms if you implement it yourself. The core protocol, however, (pkt-line framing, ref negotiation logic) can be implemented with just the specs above.
	•	Protocol Versions: The specification above describes the original Git “protocol version 1” over HTTP (sometimes called v0/v1). Git has a newer protocol version 2, which changes how refs and capabilities are exchanged (using a command/response format and the Git-Protocol: version=2 HTTP header) ￼ ￼. Version 2 can be simpler to implement in some ways, but it’s optional. Many clients and servers still use v1 by default for HTTP. You may choose to implement only v1 initially for compatibility. (The client can be forced to v1 by not sending the Git-Protocol: version=2 header in its requests.)

By following this specification – implementing the info/refs discovery, pkt-line message framing, upload-pack negotiation for fetch, receive-pack handling for push, and proper packfile transfer – you can build a functional Git server in Rust/WASM that standard Git clients can push to and clone from. This covers all essential messages and endpoint behaviors needed for Git Smart HTTP ￼ ￼. Good luck with your implementation!

Sources:
	•	Official Git documentation: Git HTTP protocol ￼ ￼, Git pack protocol ￼ ￼, Git protocol capabilities ￼ ￼, etc.
	•	Stack Overflow discussion on Git Smart HTTP (examples of client/server dialogue) ￼ ￼.
	•	Pro Git book and Git man pages for conceptual understanding of Git protocols.
