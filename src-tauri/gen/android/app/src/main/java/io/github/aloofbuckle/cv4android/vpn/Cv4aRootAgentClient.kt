package io.github.aloofbuckle.cv4android.vpn

import android.content.Context
import android.net.LocalSocket
import android.net.LocalSocketAddress
import org.json.JSONObject
import java.io.File

data class Cv4aRootAgentStatus(
    val transparentActive: Boolean,
)

object Cv4aRootAgentClient {
    fun status(context: Context): Cv4aRootAgentStatus =
        request(context, JSONObject().put("op", "status"))

    fun setTransparent(context: Context, enable: Boolean): Cv4aRootAgentStatus =
        request(
            context,
            JSONObject()
                .put("op", "set_transparent")
                .put("enable", enable),
        )

    private fun request(context: Context, request: JSONObject): Cv4aRootAgentStatus {
        val socketPath = File(context.applicationInfo.dataDir, "run/root-agent.sock")
        check(socketPath.exists()) { "Root service is not installed or running" }
        val reply = LocalSocket().use { socket ->
            socket.connect(
                LocalSocketAddress(
                    socketPath.absolutePath,
                    LocalSocketAddress.Namespace.FILESYSTEM,
                ),
            )
            socket.soTimeout = 5_000
            socket.outputStream.write((request.toString() + "\n").toByteArray(Charsets.UTF_8))
            socket.outputStream.flush()
            socket.inputStream.bufferedReader().readLine().orEmpty()
        }
        check(reply.isNotEmpty()) { "Root service returned an empty response" }
        val json = JSONObject(reply)
        check(json.optBoolean("ok")) {
            json.optString("error").takeIf { it.isNotBlank() }
                ?: "Root service rejected the request"
        }
        val status = json.optJSONObject("status") ?: error("Root service response has no status")
        return Cv4aRootAgentStatus(
            transparentActive = status.optBoolean("transparentActive"),
        )
    }
}
